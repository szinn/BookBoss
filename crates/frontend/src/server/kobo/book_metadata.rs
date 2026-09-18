//! `GET /kobo/{sync_token}/v1/library/{uuid}/metadata`
//!
//! Returns a single-element JSON array of book metadata in the Kobo wire
//! format. The Kobo device calls this to refresh a book's details outside of
//! the main library sync flow.
//!
//! Sources consulted: Komga (`KoboController.kt`, `KoboBookMetadataDto.kt`)
//! and Calibre-Web (`kobo.py :: HandleMetadataRequest`).

use std::{collections::HashMap, sync::Arc};

use axum::{
    Json,
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use bb_core::{CoreServices, book::BookToken};

use super::{KoboDevice, dto};

// ── Handler
// ─────────────────────────────────────────────────────────────────

pub async fn handle(kobo: KoboDevice, Path(params): Path<HashMap<String, String>>, core_services: Arc<CoreServices>, base_url: String) -> Response {
    let Some(uuid) = params.get("uuid") else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let Ok(token) = BookToken::from_encoded_id(uuid) else {
        return Json(Vec::<dto::KoboBookMetadata>::new()).into_response();
    };

    tracing::debug!(device_id = kobo.device.id, book_token = %token, "Retrieve book metadata");

    // Look up the book — return empty array if not found (matches Komga
    // behaviour).
    let book = match core_services.book_service.find_book_by_token(token).await {
        Ok(Some(b)) => b,
        Ok(None) => return Json(Vec::<dto::KoboBookMetadata>::new()).into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "find_book_by_token failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Load files and pick the best one for the download URL.
    let files = match core_services.book_service.files_for_book(book.id).await {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(error = ?e, "files_for_book failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let best_file = dto::select_best_file(&files);

    let series_name = if let Some(series_id) = book.series_id {
        core_services
            .book_service
            .list_all_series()
            .await
            .ok()
            .and_then(|all| all.into_iter().find(|s| s.id == series_id).map(|s| s.name))
    } else {
        None
    };

    let base = base_url.trim_end_matches('/');
    let metadata = dto::build_book_metadata(&book, best_file, &kobo.sync_token, base, series_name);

    Json(vec![metadata]).into_response()
}
