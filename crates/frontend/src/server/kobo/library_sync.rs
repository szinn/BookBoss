//! `GET /kobo/{sync_token}/v1/library/sync`
//!
//! Incremental and full-sync endpoint. The Kobo echoes back the cursor we
//! returned in the previous response's `x-kobo-synctoken` header; we decode
//! it to determine `since` and the keyset bookmark (`after_book_id`).
//!
//! # Response headers
//!
//! | Header              | Value                                              |
//! |---------------------|----------------------------------------------------|
//! | `x-kobo-synctoken`  | encoded cursor for the next request                |
//! | `x-kobo-sync`       | `"continue"` when more pages remain (absent when done) |
//!
//! Sources consulted: Komga (KoboController.kt, BookEntitlementDto.kt,
//! KoboBookMetadataDto.kt) and Calibre-Web (kobo.py).

use std::sync::Arc;

use axum::{
    Json,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::IntoResponse,
};
use bb_core::CoreServices;
use chrono::Utc;

use super::{
    KoboDevice, cursor,
    dto::{self, KoboSyncItem},
};

// ── Handler
// ─────────────────────────────────────────────────────────────────

pub async fn handle(kobo: KoboDevice, req_headers: HeaderMap, core_services: Arc<CoreServices>, base_url: String) -> Result<impl IntoResponse, StatusCode> {
    // 1. Decode sync cursor from request header (absent = full sync from
    //    start).
    let raw_cursor = req_headers.get("x-kobo-synctoken").and_then(|v| v.to_str().ok()).unwrap_or("");
    let (cursor_since, cursor_after_book_id) = cursor::decode(raw_cursor);

    // If last_synced_at is None the device sync was reset server-side; ignore
    // the Kobo's cursor so the next sync is treated as a full sync regardless
    // of what cursor the device echoes back.
    let (since, after_book_id) = if kobo.device.last_synced_at.is_none() {
        tracing::info!(device_id = kobo.device.id, "last_synced_at is None — forcing full sync");
        (None, None)
    } else {
        (cursor_since, cursor_after_book_id)
    };

    // 2. Compute which books to add / remove.
    let diff = core_services
        .device_service
        .compute_sync_diff(kobo.device.id, kobo.device.owner_id, since, after_book_id, 100)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "compute_sync_diff failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::debug!(
        device_id = kobo.device.id,
        new_books = diff.new_books.len(),
        upgraded_books = diff.upgraded_books.len(),
        refreshed_books = diff.refreshed_books.len(),
        removed_books = diff.removed_book_ids.len(),
        has_more = diff.has_more,
        "kobo library sync"
    );

    // 3. Persist DeviceBook records for this page.
    core_services.device_service.apply_sync(kobo.device.id, &diff).await.map_err(|e| {
        tracing::error!(error = ?e, "apply_sync failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 4. Fetch reading state for all books on this sync page. Pages are capped
    //    at 100 entries so the individual reads are acceptable here.
    let mut state_map = std::collections::HashMap::new();
    for entry in diff.new_books.iter().chain(diff.upgraded_books.iter()).chain(diff.refreshed_books.iter()) {
        if let Ok(Some(s)) = core_services.reading_service.get_reading_state(kobo.device.owner_id, entry.book.id).await {
            state_map.insert(entry.book.id, s);
        }
    }

    // 5. Build series lookup map for this sync page.
    let series_map: std::collections::HashMap<bb_core::book::SeriesId, String> = core_services
        .book_service
        .list_all_series()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|s| (s.id, s.name))
        .collect();

    // 6. Build response body.
    let base = base_url.trim_end_matches('/');
    let t = &kobo.sync_token;
    let mut items: Vec<KoboSyncItem> =
        Vec::with_capacity(diff.removed_book_ids.len() + diff.new_books.len() + diff.upgraded_books.len() + diff.refreshed_books.len());

    // Removed books → ChangedEntitlement with IsRemoved: true.
    for &book_id in &diff.removed_book_ids {
        items.push(dto::build_removed_entitlement(book_id));
    }

    // New books → NewEntitlement.
    for entry in &diff.new_books {
        let rs = state_map.get(&entry.book.id);
        let series = entry.book.series_id.and_then(|id| series_map.get(&id)).cloned();
        items.push(dto::build_new_entitlement(entry, t, base, rs, series));
    }

    // Upgraded (format changed) and refreshed (metadata changed) books →
    // ChangedEntitlement so the device replaces its existing copy.
    for entry in diff.upgraded_books.iter().chain(diff.refreshed_books.iter()) {
        let rs = state_map.get(&entry.book.id);
        let series = entry.book.series_id.and_then(|id| series_map.get(&id)).cloned();
        items.push(dto::build_changed_entitlement(entry, t, base, rs, series));
    }

    // 6. Compute cursor for next request.
    //    - Mid-pagination: preserve `since`, advance keyset bookmark.
    //    - Final page: advance `since` to now so next sync is incremental.
    let last_book_id = [diff.new_books.last(), diff.upgraded_books.last(), diff.refreshed_books.last()]
        .into_iter()
        .flatten()
        .map(|e| e.book.id)
        .max();

    let next_cursor = if diff.has_more {
        cursor::encode(since, last_book_id)
    } else {
        cursor::encode(Some(Utc::now()), None)
    };

    // 7. Build response headers. x-kobo-synctoken: always present (cursor for
    //    next call). x-kobo-sync:      "continue" only when more pages remain;
    //    absent when done.
    let next_cursor_hv = HeaderValue::try_from(next_cursor).expect("cursor contains only ASCII digits and colons");

    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(HeaderName::from_static("x-kobo-synctoken"), next_cursor_hv);
    if diff.has_more {
        resp_headers.insert(HeaderName::from_static("x-kobo-sync"), HeaderValue::from_static("continue"));
    }

    Ok((resp_headers, Json(items)))
}
