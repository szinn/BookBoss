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
    book::BookToken,
    reading::{DeviceReadingState, ReadStatus},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;

use super::KoboDevice;

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
        Some(s) => build_kobo_state(&s),
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

    // Extract position and progress from CurrentBookmark. Apply Finished
    // override: clear location and force 100% when Finished.
    let (progress_bps, position_type, position_token) = if finished {
        (Some(10000u16), None, None)
    } else if let Some(bm) = item.current_bookmark {
        #[allow(clippy::cast_sign_loss, reason = "progress_percent is always positive")]
        let progress_bps = (bm.progress_percent * 100.0).round() as u16;
        let (pt, pv) = bm
            .location
            .filter(|l| !l.kind.is_empty() && !l.value.is_empty())
            .map(|l| (Some(l.kind), Some(l.value)))
            .unwrap_or_default();
        (Some(progress_bps), pt, pv)
    } else {
        (None, None, None)
    };

    let stats = item.statistics.unwrap_or_default();

    tracing::debug!(
        device_id = kobo.device.id,
        book_token = %token,
        status = ?new_status,
        progress_bps = progress_bps,
        position_type = position_type,
        position_token = position_token,
        stats = ?stats,
        last_modified = ?status_info.last_modified,
        "Updating book status"
    );

    if let Err(e) = core_services
        .reading_service
        .sync_device_state(
            kobo.device.owner_id,
            book.id,
            new_status,
            DeviceReadingState {
                progress_bps,
                position_type,
                position_token,
                spent_reading_minutes: stats.spent_reading_minutes,
                remaining_time_minutes: stats.remaining_time_minutes,
                last_progress_at: status_info.last_modified,
                ..DeviceReadingState::default()
            },
        )
        .await
    {
        tracing::error!(error = ?e, "sync_device_state failed");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    Json(json!({ "RequestResult": "Success", "UpdateResults": [] })).into_response()
}

// ── Mapping helpers
// ────────────────────────────────────────────────────────

/// Maps a stored `UserBookMetadata` to the Kobo state wire format.
pub(super) fn build_kobo_state(state: &bb_core::reading::UserBookMetadata) -> serde_json::Value {
    let kobo_status = read_status_to_kobo_status(state.read_status);
    let last_modified = state.last_progress_at.unwrap_or_else(Utc::now).to_rfc3339();

    let (progress, location) = match state.read_status {
        ReadStatus::Read => (100.0f64, None),
        ReadStatus::Unread => (0.0f64, None),
        _ => {
            let p = state.progress_percentage.map_or(0.0, |v| f64::from(v) / 100.0);
            let loc = match (&state.position_type, &state.position_token) {
                (Some(t), Some(v)) if !t.is_empty() && !v.is_empty() => Some(json!({
                    "Type": t,
                    "Value": v,
                })),
                _ => None,
            };
            (p, loc)
        }
    };

    let mut bookmark = json!({
        "ProgressPercent": progress,
        "ContentSourceProgressPercent": progress,
    });
    if let Some(loc) = location {
        bookmark["Location"] = loc;
    }

    let mut obj = json!({
        "CurrentBookmark": bookmark,
        "StatusInfo": {
            "Status": kobo_status,
            "LastModified": last_modified,
        },
    });

    if state.spent_reading_minutes.is_some() || state.remaining_time_minutes.is_some() {
        obj["Statistics"] = json!({
            "SpentReadingMinutes": state.spent_reading_minutes,
            "RemainingTimeMinutes": state.remaining_time_minutes,
        });
    }

    obj
}
