//! `KoReaderUser` extractor for KOReader sync endpoints.
//!
//! Authenticates via `x-auth-user` and `x-auth-key` headers.
//! `x-auth-key` must equal `md5(opds_password)`.
//! Returns 401 if credentials are missing or invalid, 403 if the user lacks
//! the `OpdsAccess` capability.

use std::sync::Arc;

use axum::{
    Extension,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use bb_core::{CoreServices, types::Capability, user::User};
use md5::{Digest, Md5};

pub struct KoReaderUser {
    pub user: User,
}

impl<S: Send + Sync> FromRequestParts<S> for KoReaderUser {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // 1. Extract custom auth headers (owned so we can pass parts mutably
        //    later).
        let username = parts
            .headers
            .get("x-auth-user")
            .and_then(|v| v.to_str().ok())
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let auth_key = parts
            .headers
            .get("x-auth-key")
            .and_then(|v| v.to_str().ok())
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .ok_or(StatusCode::UNAUTHORIZED)?;

        // 2. Resolve CoreServices.
        let Extension(core_services) = Extension::<Arc<CoreServices>>::from_request_parts(parts, state)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // 3. Look up user by username.
        let user = core_services
            .user_service
            .find_by_username(&username)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::UNAUTHORIZED)?;

        // 4. Check OpdsAccess capability.
        if !user.has_capability(Capability::OpdsAccess) {
            return Err(StatusCode::FORBIDDEN);
        }

        // 5. Get the OPDS password (decrypted plaintext, read-only). Returns
        //    None if the user hasn't set up an OPDS password yet → 401. We
        //    intentionally do NOT call get_or_create_password here — auth must
        //    be read-only. Creating a password silently during an auth check
        //    would be a surprising write side effect.
        let opds_password = core_services
            .opds_service
            .get_password(&user)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::UNAUTHORIZED)?;

        // 6. Compute md5(opds_password) and compare to x-auth-key.
        let expected_key = {
            use std::fmt::Write;
            Md5::digest(opds_password.as_bytes()).iter().fold(String::with_capacity(32), |mut s, b| {
                let _ = write!(s, "{b:02x}");
                s
            })
        };

        if auth_key != expected_key {
            return Err(StatusCode::UNAUTHORIZED);
        }

        Ok(Self { user })
    }
}
