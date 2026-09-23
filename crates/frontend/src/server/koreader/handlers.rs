//! KOReader sync protocol handlers.
//!
//! All endpoints mounted under `/koreader/`.

use std::sync::Arc;

use axum::{Extension, Json, extract::Path, http::StatusCode, response::Response};
use bb_core::{
    CoreServices,
    reading::{DeviceReadingState, ReadStatus},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::extractor::KoReaderUser;

// ── DTOs ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct HealthResponse {
    pub state: &'static str,
}

#[derive(Deserialize)]
pub struct ProgressPushBody {
    pub document: String,
    pub progress: String,
    pub percentage: f64,
    pub device: Option<String>,
    #[allow(dead_code)]
    pub device_id: Option<String>,
}

#[derive(Serialize)]
pub struct ProgressResponse {
    pub document: String,
    pub progress: String,
    pub percentage: f64,
    pub device: String,
    pub timestamp: i64,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// `GET /koreader/healthcheck` — no auth required.
pub async fn healthcheck() -> Json<HealthResponse> {
    Json(HealthResponse { state: "OK" })
}

/// `POST /koreader/users/create` — registration disabled, always 402.
pub async fn users_create() -> StatusCode {
    StatusCode::PAYMENT_REQUIRED
}

/// `GET /koreader/users/auth` — returns 200 if credentials are valid.
/// The `KoReaderUser` extractor handles all auth logic; if we reach this
/// handler, auth passed.
pub async fn users_auth(_user: KoReaderUser) -> StatusCode {
    StatusCode::OK
}

/// `PUT /koreader/syncs/progress` — push reading position.
pub async fn syncs_progress_push(
    koreader_user: KoReaderUser,
    Extension(core_services): Extension<Arc<CoreServices>>,
    Json(body): Json<ProgressPushBody>,
) -> Response {
    use axum::response::IntoResponse;

    let user_id = koreader_user.user.id;

    // 1. Resolve document digest → BookId.
    let book_id = match core_services.koreader_service.find_book_by_digest(&body.document).await {
        Ok(Some(id)) => {
            tracing::debug!(document = %body.document, book_id = id, "KOReader push: resolved document hash");
            id
        }
        Ok(None) => {
            tracing::debug!(document = %body.document, user_id = ?user_id, "KOReader push: document hash not found");
            return StatusCode::NOT_FOUND.into_response();
        }
        Err(e) => {
            tracing::error!(document = %body.document, error = %e, "KOReader push: hash lookup error");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // 2. Load current reading state to determine status transitions.
    let current_status = match core_services.reading_service.get_reading_state(user_id, book_id).await {
        Ok(state) => {
            let status = state.map_or(ReadStatus::Unread, |s| s.read_status);
            tracing::debug!(book_id, ?status, "KOReader push: current reading status");
            status
        }
        Err(e) => {
            tracing::error!(document = %body.document, book_id, error = %e, "KOReader push: reading state lookup error");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // 3. Determine target ReadStatus:
    //    - percentage == 1.0                     → Read
    //    - 0.0 < percentage < 1.0 + Unread       → Reading
    //    - otherwise                              → keep current status
    let target_status = if (body.percentage - 1.0).abs() < f64::EPSILON {
        ReadStatus::Read
    } else if body.percentage > 0.0 && current_status == ReadStatus::Unread {
        ReadStatus::Reading
    } else {
        current_status
    };

    // 4. Persist via sync_device_state. progress_bps is 0–10000 (basis points
    //    of 100%). Capture timestamp once so the stored value and the response
    //    agree.
    let now = Utc::now();
    #[allow(clippy::cast_sign_loss)] // value is clamped to [0.0, 1.0] * 10_000.0 → always non-negative
    let progress_bps = (body.percentage.clamp(0.0, 1.0) * 10_000.0).round() as u16;
    match core_services
        .reading_service
        .sync_device_state(
            user_id,
            book_id,
            target_status,
            DeviceReadingState {
                progress_bps: Some(progress_bps),
                position_type: Some("KoReader".to_string()),
                position_token: Some(body.progress.clone()),
                last_progress_at: Some(now),
                ..DeviceReadingState::default()
            },
        )
        .await
    {
        Ok(_) => {}
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    Json(ProgressResponse {
        document: body.document,
        progress: body.progress,
        percentage: body.percentage,
        device: body.device.unwrap_or_default(),
        timestamp: now.timestamp(),
    })
    .into_response()
}

/// `GET /koreader/syncs/progress/:document` — pull latest position.
pub async fn syncs_progress_pull(
    koreader_user: KoReaderUser,
    Extension(core_services): Extension<Arc<CoreServices>>,
    Path(document): Path<String>,
) -> Response {
    use axum::response::IntoResponse;

    // 1. Resolve document digest → BookId.
    let book_id = match core_services.koreader_service.find_book_by_digest(&document).await {
        Ok(Some(id)) => {
            tracing::debug!(document = %document, book_id = id, "KOReader pull: resolved document hash");
            id
        }
        Ok(None) => {
            tracing::debug!(document = %document, "KOReader pull: document hash not found");
            return StatusCode::NOT_FOUND.into_response();
        }
        Err(e) => {
            tracing::error!(document = %document, error = %e, "KOReader pull: hash lookup error");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // 2. Load UserBookMetadata. If no reading state exists yet, return unread
    //    defaults (percentage 0, empty progress token) rather than 404 — the
    //    book is known to BookBoss, just not yet started.
    let metadata = match core_services.reading_service.get_reading_state(koreader_user.user.id, book_id).await {
        Ok(m) => {
            tracing::debug!(book_id, has_state = m.is_some(), "KOReader pull: reading state");
            m
        }
        Err(e) => {
            tracing::error!(document = %document, book_id, error = %e, "KOReader pull: reading state lookup error");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let (progress, percentage, timestamp) = metadata.map_or((String::new(), 0.0, 0), |m| {
        let pct = f64::from(m.progress_percentage.unwrap_or(0)) / 10_000.0;
        let ts = m.last_progress_at.map_or(0, |t| t.timestamp());
        (m.position_token.unwrap_or_default(), pct, ts)
    });

    Json(ProgressResponse {
        document,
        progress,
        percentage,
        device: String::new(),
        timestamp,
    })
    .into_response()
}
