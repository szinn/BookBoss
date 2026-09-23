//! `GET /kobo/{sync_token}/v1/library/{uuid}/state`
//! `PUT /kobo/{sync_token}/v1/library/{uuid}/state`
//!
//! Handles per-book reading state sync with the Kobo device.
//!
//! # PUT
//! The body is a single JSON state object carrying reading progress
//! (position, percent, time stats, status). If the object carries
//! `"DeleteEntitlement": true` the book is removed from the device's sync
//! list instead (same effect as the DELETE endpoint).
//!
//! # GET
//! Returns the stored reading state for the book so the device can restore
//! position. Returns `[{}]` when no state has been saved yet — the Kobo
//! treats that as no saved position and does not crash.
//!
//! # Status mappings (Kobo ↔ internal)
//! | Kobo `StatusInfo.Status` | `ReadStatus` |
//! |--------------------------|--------------|
//! | `"ReadyToRead"`          | `Unread`     |
//! | `"Reading"`              | `Reading`    |
//! | `"Finished"`             | `Read`       |
//!
//! # Finished override rule
//! When the Kobo reports `Finished`, the position token and type are cleared
//! (not stored) and progress is forced to 100%. Conversely, when returning
//! state for a `Read` book, the position blob is omitted and percent is 1.0.

use std::{collections::HashMap, sync::Arc};

use axum::{Json, body::Bytes, extract::Path, http::StatusCode, response::IntoResponse};
use bb_core::{
    CoreServices,
    book::{Book, BookToken},
    reading::{DeviceReadingState, ReadStatus, UserBookMetadata},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;

use super::{KoboDevice, dto::book_uuid_from_token};

// ── Deserialization types
// ──────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct KoboLocation {
    source: String,
    #[serde(rename = "Type")]
    kind: String,
    value: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct KoboBookmark {
    location: Option<KoboLocation>,
    progress_percent: f64,
    content_source_progress_percent: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct KoboStatistics {
    spent_reading_minutes: Option<i32>,
    remaining_time_minutes: Option<i32>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct KoboStatusInfo {
    status: String,
    last_modified: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct StateItem {
    delete_entitlement: bool,
    current_bookmark: Option<KoboBookmark>,
    statistics: Option<KoboStatistics>,
    status_info: Option<KoboStatusInfo>,
}

// ── Status helpers
// ─────────────────────────────────────────────────────────

fn kobo_status_to_read_status(s: &str) -> ReadStatus {
    match s {
        "Finished" => ReadStatus::Read,
        "Reading" => ReadStatus::Reading,
        _ => ReadStatus::Unread,
    }
}

fn read_status_to_kobo_status(s: ReadStatus) -> &'static str {
    match s {
        ReadStatus::Read => "Finished",
        ReadStatus::Reading | ReadStatus::Paused | ReadStatus::Rereading => "Reading",
        _ => "ReadyToRead",
    }
}

// ── GET handler
// ────────────────────────────────────────────────────────────

pub(super) async fn handle_get(kobo: KoboDevice, Path(params): Path<HashMap<String, String>>, core_services: Arc<CoreServices>) -> impl IntoResponse {
    let Some(uuid) = params.get("uuid") else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let Ok(token) = BookToken::from_encoded_id(uuid) else {
        return Json(json!([])).into_response();
    };

    tracing::debug!(device_id = kobo.device.id, book_token = %token, "Retrieve book state");

    let book = match core_services.book_service.find_book_by_token(token).await {
        Ok(Some(b)) => b,
        Ok(None) => return Json(json!([])).into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "find_book_by_token failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let state = match core_services.reading_service.get_reading_state(kobo.device.owner_id, book.id).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = ?e, "get_reading_state failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let item = match state {
        None => json!({}),
        Some(s) => build_kobo_state(&book, &s),
    };

    Json(json!([item])).into_response()
}

// ── PUT handler
// ────────────────────────────────────────────────────────────

pub(super) async fn handle_put(
    kobo: KoboDevice,
    Path(params): Path<HashMap<String, String>>,
    core_services: Arc<CoreServices>,
    body: Bytes,
) -> impl IntoResponse {
    // The Kobo sends { "ReadingStates": [ { ...state... } ] }; unwrap to the
    // first element before deserializing into StateItem.
    #[derive(Deserialize, Default)]
    #[serde(rename_all = "PascalCase", default)]
    struct ReadingStatesWrapper {
        reading_states: Vec<StateItem>,
    }
    let item: StateItem = match serde_json::from_slice::<ReadingStatesWrapper>(&body) {
        Ok(mut w) if !w.reading_states.is_empty() => w.reading_states.swap_remove(0),
        Ok(_) => {
            tracing::warn!("kobo state PUT body had empty ReadingStates array");
            return Json(json!({ "RequestResult": "Success", "UpdateResults": [] })).into_response();
        }
        Err(e) => {
            tracing::warn!(error = ?e, body = %String::from_utf8_lossy(&body), "failed to parse state PUT body");
            return Json(json!({ "RequestResult": "Success", "UpdateResults": [] })).into_response();
        }
    };
    let Some(uuid) = params.get("uuid") else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let Ok(token) = BookToken::from_encoded_id(uuid) else {
        return StatusCode::OK.into_response();
    };

    tracing::debug!(device_id = kobo.device.id, book_token = %token, state_info = ?item, "set book state");

    let book = match core_services.book_service.find_book_by_token(token).await {
        Ok(Some(b)) => b,
        Ok(None) => return StatusCode::OK.into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "find_book_by_token failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // DeleteEntitlement: remove the book from the device sync list.
    if item.delete_entitlement {
        if let Err(e) = core_services.device_service.remove_book_from_device(kobo.device.id, book.id).await {
            tracing::error!(error = ?e, "remove_book_from_device failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }

        tracing::debug!(device_id = kobo.device.id, book_id = book.id, "kobo delete entitlement via state");

        return Json(json!({ "RequestResult": "Success", "UpdateResults": [] })).into_response();
    }

    // No status info means nothing to persist.
    let Some(status_info_val) = item.status_info else {
        return Json(json!({ "RequestResult": "Success", "UpdateResults": [] })).into_response();
    };

    let status_info = status_info_val;
    let new_status = kobo_status_to_read_status(&status_info.status);
    let finished = matches!(new_status, ReadStatus::Read);

    let report = device_state_from_kobo(finished, item.current_bookmark, &item.statistics.unwrap_or_default(), status_info.last_modified);

    tracing::debug!(
        device_id = kobo.device.id,
        book_token = %token,
        status = ?new_status,
        report = ?report,
        "Updating book status"
    );

    if let Err(e) = core_services
        .reading_service
        .sync_device_state(kobo.device.owner_id, book.id, new_status, report)
        .await
    {
        tracing::error!(error = ?e, "sync_device_state failed");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    Json(json!({ "RequestResult": "Success", "UpdateResults": [] })).into_response()
}

// ── Mapping helpers
// ────────────────────────────────────────────────────────

/// Converts a Kobo percentage (0–100) to basis points.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation, reason = "clamped to 0–100")]
fn percent_to_bps(percent: f64) -> u16 {
    (percent.clamp(0.0, 100.0) * 100.0).round() as u16
}

/// Maps a Kobo state PUT to a `DeviceReadingState`.
///
/// When the Kobo reports `Finished` the location is cleared and progress is
/// forced to 100%. A location is only kept when `Type` and `Value` are both
/// present; `Source` (the content file a `KoboSpan` belongs to) is stored
/// alongside so the position can be restored on another device.
fn device_state_from_kobo(finished: bool, bookmark: Option<KoboBookmark>, stats: &KoboStatistics, last_modified: Option<DateTime<Utc>>) -> DeviceReadingState {
    let mut report = DeviceReadingState {
        spent_reading_minutes: stats.spent_reading_minutes,
        remaining_time_minutes: stats.remaining_time_minutes,
        last_progress_at: last_modified,
        ..DeviceReadingState::default()
    };

    if finished {
        report.progress_bps = Some(10000);
    } else if let Some(bm) = bookmark {
        report.progress_bps = Some(percent_to_bps(bm.progress_percent));
        report.content_source_progress_bps = bm.content_source_progress_percent.map(percent_to_bps);
        if let Some(loc) = bm.location.filter(|l| !l.kind.is_empty() && !l.value.is_empty()) {
            report.position_type = Some(loc.kind);
            report.position_token = Some(loc.value);
            report.position_source = Some(loc.source).filter(|s| !s.is_empty());
        }
    }

    report
}

/// Maps a stored `UserBookMetadata` to the Kobo state wire format.
///
/// Shape follows Calibre-Web (`get_kobo_reading_state_response`) and Komga
/// (`ReadingStateDto`). `Location` is only sent when `Source` is known: a
/// `KoboSpan` value is only unique within its content file.
pub(super) fn build_kobo_state(book: &Book, state: &UserBookMetadata) -> serde_json::Value {
    let kobo_status = read_status_to_kobo_status(state.read_status);
    let last_modified = state.last_progress_at.unwrap_or(book.updated_at).to_rfc3339();

    let mut bookmark = json!({ "LastModified": last_modified });
    match state.read_status {
        ReadStatus::Read => {
            bookmark["ProgressPercent"] = json!(100.0);
        }
        ReadStatus::Unread => {
            bookmark["ProgressPercent"] = json!(0.0);
        }
        _ => {
            if let Some(p) = state.progress_percentage {
                bookmark["ProgressPercent"] = json!(f64::from(p) / 100.0);
            }
            if let Some(p) = state.content_source_progress_percentage {
                bookmark["ContentSourceProgressPercent"] = json!(f64::from(p) / 100.0);
            }
            if let (Some(t), Some(v), Some(src)) = (&state.position_type, &state.position_token, &state.position_source)
                && !t.is_empty()
                && !v.is_empty()
                && !src.is_empty()
            {
                bookmark["Location"] = json!({ "Value": v, "Type": t, "Source": src });
            }
        }
    }

    let mut statistics = json!({ "LastModified": last_modified });
    if let Some(m) = state.spent_reading_minutes {
        statistics["SpentReadingMinutes"] = json!(m);
    }
    if let Some(m) = state.remaining_time_minutes {
        statistics["RemainingTimeMinutes"] = json!(m);
    }

    let times_started = if state.read_status == ReadStatus::Unread {
        0
    } else {
        state.times_read.max(1)
    };

    json!({
        "EntitlementId": book_uuid_from_token(book.token),
        "Created": book.created_at.to_rfc3339(),
        "LastModified": last_modified,
        "PriorityTimestamp": last_modified,
        "StatusInfo": {
            "LastModified": last_modified,
            "Status": kobo_status,
            "TimesStartedReading": times_started,
        },
        "Statistics": statistics,
        "CurrentBookmark": bookmark,
    })
}

#[cfg(test)]
mod tests {
    use bb_core::book::BookStatus;
    use chrono::TimeZone;

    use super::*;

    fn book() -> Book {
        let created = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        Book {
            id: 1,
            version: 1,
            token: BookToken::new(1),
            title: "Dead Line".to_string(),
            status: BookStatus::Available,
            description: None,
            published_date: None,
            language: None,
            series_id: None,
            series_number: None,
            publisher_id: None,
            page_count: None,
            rating: None,
            metadata_source: None,
            has_cover: false,
            sidecar_fingerprint: None,
            created_at: created,
            updated_at: created,
        }
    }

    fn reading_state() -> UserBookMetadata {
        UserBookMetadata {
            user_id: 1,
            book_id: 1,
            read_status: ReadStatus::Reading,
            progress_percentage: Some(3700),
            position_type: Some("KoboSpan".to_string()),
            position_token: Some("kobo.12.1".to_string()),
            position_source: Some("OEBPS/ch03.xhtml".to_string()),
            content_source_progress_percentage: Some(4250),
            last_progress_at: Some(Utc.with_ymd_and_hms(2026, 9, 23, 18, 0, 0).unwrap()),
            spent_reading_minutes: Some(95),
            remaining_time_minutes: Some(160),
            personal_rating: None,
            times_read: 0,
            date_started: None,
            date_finished: None,
            last_opened_at: None,
            notes: None,
        }
    }

    fn parse(body: serde_json::Value) -> StateItem {
        serde_json::from_value(body).unwrap()
    }

    fn reading_item() -> StateItem {
        parse(json!({
            "CurrentBookmark": {
                "ProgressPercent": 37.0,
                "ContentSourceProgressPercent": 42.5,
                "Location": { "Source": "OEBPS/ch03.xhtml", "Type": "KoboSpan", "Value": "kobo.12.1" },
            },
            "Statistics": { "SpentReadingMinutes": 95, "RemainingTimeMinutes": 160 },
            "StatusInfo": { "Status": "Reading", "LastModified": "2026-09-23T18:00:00Z" },
        }))
    }

    #[test]
    fn put_keeps_location_source_and_content_progress() {
        let item = reading_item();
        let report = device_state_from_kobo(false, item.current_bookmark, &item.statistics.unwrap_or_default(), None);

        assert_eq!(report.progress_bps, Some(3700));
        assert_eq!(report.content_source_progress_bps, Some(4250));
        assert_eq!(report.position_type.as_deref(), Some("KoboSpan"));
        assert_eq!(report.position_token.as_deref(), Some("kobo.12.1"));
        assert_eq!(report.position_source.as_deref(), Some("OEBPS/ch03.xhtml"));
        assert_eq!(report.spent_reading_minutes, Some(95));
        assert_eq!(report.remaining_time_minutes, Some(160));
    }

    #[test]
    fn put_finished_clears_location() {
        let item = reading_item();
        let report = device_state_from_kobo(true, item.current_bookmark, &KoboStatistics::default(), None);

        assert_eq!(report.progress_bps, Some(10000));
        assert_eq!(report.content_source_progress_bps, None);
        assert_eq!(report.position_token, None);
        assert_eq!(report.position_source, None);
    }

    #[test]
    fn put_without_location_value_stores_no_position() {
        let item = parse(json!({
            "CurrentBookmark": { "ProgressPercent": 5.0, "Location": { "Source": "OEBPS/ch01.xhtml" } },
        }));
        let report = device_state_from_kobo(false, item.current_bookmark, &KoboStatistics::default(), None);

        assert_eq!(report.progress_bps, Some(500));
        assert_eq!(report.position_type, None);
        assert_eq!(report.position_source, None);
    }

    #[test]
    fn state_includes_full_location_and_timestamps() {
        let book = book();
        let v = build_kobo_state(&book, &reading_state());
        let last_modified = "2026-09-23T18:00:00+00:00";

        assert_eq!(v["EntitlementId"], book_uuid_from_token(book.token));
        assert_eq!(v["Created"], "2026-01-01T00:00:00+00:00");
        assert_eq!(v["LastModified"], last_modified);
        assert_eq!(v["PriorityTimestamp"], last_modified);
        assert_eq!(v["StatusInfo"]["Status"], "Reading");
        assert_eq!(v["StatusInfo"]["TimesStartedReading"], 1);
        assert_eq!(v["Statistics"]["SpentReadingMinutes"], 95);

        let bm = &v["CurrentBookmark"];
        assert_eq!(bm["LastModified"], last_modified);
        assert_eq!(bm["ProgressPercent"], 37.0);
        assert_eq!(bm["ContentSourceProgressPercent"], 42.5);
        assert_eq!(
            bm["Location"],
            json!({ "Value": "kobo.12.1", "Type": "KoboSpan", "Source": "OEBPS/ch03.xhtml" })
        );
    }

    #[test]
    fn state_omits_location_without_source() {
        let state = UserBookMetadata {
            position_source: None,
            ..reading_state()
        };
        let v = build_kobo_state(&book(), &state);

        assert!(v["CurrentBookmark"].get("Location").is_none());
        assert_eq!(v["CurrentBookmark"]["ProgressPercent"], 37.0);
    }

    #[test]
    fn state_for_finished_book_has_full_progress_and_no_location() {
        let state = UserBookMetadata {
            read_status: ReadStatus::Read,
            ..reading_state()
        };
        let v = build_kobo_state(&book(), &state);

        assert_eq!(v["StatusInfo"]["Status"], "Finished");
        assert_eq!(v["CurrentBookmark"]["ProgressPercent"], 100.0);
        assert!(v["CurrentBookmark"].get("Location").is_none());
    }
}
