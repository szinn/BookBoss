//! `KoboDevice` extractor for Kobo sync endpoints.
//!
//! The Kobo device authenticates by including its sync token in the URL path
//! (`/kobo/{sync_token}/...`). This extractor reconstructs the full
//! `DV_`-prefixed `DeviceToken`, looks up the device, and returns 401 if the
//! token is absent or unknown.

use std::{collections::HashMap, str::FromStr, sync::Arc};

use axum::{
    Extension,
    extract::{FromRequestParts, Path},
    http::{StatusCode, request::Parts},
};
use bb_core::{
    CoreServices,
    device::{Device, DeviceToken},
};

/// Holds the resolved device for a Kobo sync request.
#[derive(Clone)]
pub struct KoboDevice {
    pub device: Device,
    /// The raw sync token as it appears in the URL (without the `DV_` prefix).
    pub sync_token: String,
}

impl<S: Send + Sync> FromRequestParts<S> for KoboDevice {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // 1. Extract named path parameters — all Kobo routes contain
        //    `sync_token`.
        let Path(params) = Path::<HashMap<String, String>>::from_request_parts(parts, state)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        let sync_token = params.get("sync_token").ok_or(StatusCode::UNAUTHORIZED)?.clone();

        // 2. Reconstruct the full device token by prepending the `DV_` prefix.
        let full_token = format!("DV_{sync_token}");
        let device_token = DeviceToken::from_str(&full_token).map_err(|_| StatusCode::UNAUTHORIZED)?;

        // 3. Resolve CoreServices from the request extension layer.
        let Extension(core_services) = Extension::<Arc<CoreServices>>::from_request_parts(parts, state)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // 4. Look up the device — the token itself is the auth credential.
        let device = core_services
            .device_service
            .find_device_by_token(device_token)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::UNAUTHORIZED)?;

        Ok(Self { device, sync_token })
    }
}
