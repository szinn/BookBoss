//! `OpdsUser` extractor for OPDS endpoints.
//!
//! Authenticates via HTTP Basic Auth using the user's BookBoss username and
//! their OPDS-specific password (stored AES-256-GCM encrypted in
//! `user_settings`).
//!
//! Checks run in this order: credentials parse, user lookup, password
//! verification, then the `OpdsAccess` capability. The password is verified
//! *before* the capability check so that an unauthenticated caller cannot
//! tell which accounts exist or hold `OpdsAccess` (wrong password is always
//! 401, never 403).
//!
//! Every 401 carries `WWW-Authenticate: Basic realm="BookBoss"` so OPDS
//! clients know to prompt for credentials. 403 (correct password, no
//! `OpdsAccess`) and 500 deliberately carry no challenge, otherwise clients
//! would re-prompt for credentials that are already correct.

use std::sync::Arc;

use axum::{
    Extension,
    extract::FromRequestParts,
    http::{HeaderMap, HeaderValue, StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use bb_core::{CoreServices, types::Capability, user::User};

/// Value of the `WWW-Authenticate` header sent with every OPDS 401.
const OPDS_CHALLENGE: &str = r#"Basic realm="BookBoss""#;

/// Holds the authenticated user for an OPDS request.
#[derive(Clone)]
pub struct OpdsUser {
    pub user: User,
}

/// Why an OPDS request was rejected.
#[derive(Debug, PartialEq, Eq)]
pub enum OpdsAuthRejection {
    /// Missing, malformed or invalid credentials. Responds 401 with a
    /// challenge.
    Unauthorized,
    /// Valid credentials but no `OpdsAccess` capability. Responds 403.
    Forbidden,
    /// Unexpected internal failure. Responds 500.
    Internal,
}

impl IntoResponse for OpdsAuthRejection {
    fn into_response(self) -> Response {
        match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, [(header::WWW_AUTHENTICATE, HeaderValue::from_static(OPDS_CHALLENGE))]).into_response(),
            Self::Forbidden => StatusCode::FORBIDDEN.into_response(),
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}

/// Parses `Authorization: Basic <base64(user:pass)>` into `(username,
/// password)`.
///
/// The scheme is matched case-insensitively and the split is on the first `:`
/// so passwords may contain colons. Every failure is `Unauthorized`.
fn parse_basic(headers: &HeaderMap) -> Result<(String, String), OpdsAuthRejection> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(OpdsAuthRejection::Unauthorized)?;

    let (scheme, encoded) = auth_header.split_once(' ').ok_or(OpdsAuthRejection::Unauthorized)?;
    if !scheme.eq_ignore_ascii_case("basic") {
        return Err(OpdsAuthRejection::Unauthorized);
    }

    let decoded = STANDARD.decode(encoded.trim()).map_err(|_| OpdsAuthRejection::Unauthorized)?;
    let credentials = String::from_utf8(decoded).map_err(|_| OpdsAuthRejection::Unauthorized)?;

    let (username, password) = credentials.split_once(':').ok_or(OpdsAuthRejection::Unauthorized)?;
    if username.is_empty() || password.is_empty() {
        return Err(OpdsAuthRejection::Unauthorized);
    }

    Ok((username.to_owned(), password.to_owned()))
}

/// Credential lookups needed to authenticate an OPDS request.
trait OpdsCredentialStore {
    async fn find_user(&self, username: &str) -> Result<Option<User>, bb_core::Error>;
    async fn verify_password(&self, user: &User, password: &str) -> Result<bool, bb_core::Error>;
}

impl OpdsCredentialStore for CoreServices {
    async fn find_user(&self, username: &str) -> Result<Option<User>, bb_core::Error> {
        self.user_service.find_by_username(username).await
    }

    async fn verify_password(&self, user: &User, password: &str) -> Result<bool, bb_core::Error> {
        self.opds_service.verify_password(user, password).await
    }
}

/// Authenticates an OPDS request from its headers.
async fn authenticate(headers: &HeaderMap, store: &impl OpdsCredentialStore) -> Result<User, OpdsAuthRejection> {
    // 1. Parse Basic credentials.
    let (username, password) = parse_basic(headers)?;

    // 2. Look up user by username.
    let user = store
        .find_user(&username)
        .await
        .map_err(|_| OpdsAuthRejection::Internal)?
        .ok_or(OpdsAuthRejection::Unauthorized)?;

    // 3. Verify OPDS password before anything else about the account is
    //    revealed.
    let valid = store.verify_password(&user, &password).await.map_err(|_| OpdsAuthRejection::Internal)?;
    if !valid {
        return Err(OpdsAuthRejection::Unauthorized);
    }

    // 4. Check OpdsAccess capability.
    if !user.has_capability(Capability::OpdsAccess) {
        return Err(OpdsAuthRejection::Forbidden);
    }

    Ok(user)
}

impl<S: Send + Sync> FromRequestParts<S> for OpdsUser {
    type Rejection = OpdsAuthRejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Extension(core_services) = Extension::<Arc<CoreServices>>::from_request_parts(parts, state)
            .await
            .map_err(|_| OpdsAuthRejection::Internal)?;

        let user = authenticate(&parts.headers, &*core_services).await?;
        Ok(Self { user })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use bb_core::{types::EmailAddress, user::UserToken};
    use chrono::Utc;

    use super::*;

    struct FakeStore {
        user: Option<User>,
        password: &'static str,
        fail: bool,
    }

    #[allow(clippy::unused_async_trait_impl)]
    impl OpdsCredentialStore for FakeStore {
        async fn find_user(&self, _username: &str) -> Result<Option<User>, bb_core::Error> {
            if self.fail {
                return Err(bb_core::Error::Infrastructure("boom".into()));
            }
            Ok(self.user.clone())
        }

        async fn verify_password(&self, _user: &User, password: &str) -> Result<bool, bb_core::Error> {
            if self.fail {
                return Err(bb_core::Error::Infrastructure("boom".into()));
            }
            Ok(password == self.password)
        }
    }

    fn user_with_opds_access() -> User {
        User {
            id: 1,
            version: 1,
            token: UserToken::new(1),
            username: "alice".to_string(),
            full_name: "Alice".to_string(),
            password_hash: String::new(),
            email_address: EmailAddress::new("alice@example.com").unwrap(),
            capabilities: HashSet::from([Capability::OpdsAccess]),
            change_password_on_login: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn user_without_opds_access() -> User {
        User {
            capabilities: HashSet::new(),
            ..user_with_opds_access()
        }
    }

    fn basic(user: &str, pass: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        let encoded = STANDARD.encode(format!("{user}:{pass}"));
        headers.insert(header::AUTHORIZATION, format!("Basic {encoded}").parse().unwrap());
        headers
    }

    fn raw_auth(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, value.parse().unwrap());
        headers
    }

    fn store(user: Option<User>) -> FakeStore {
        FakeStore {
            user,
            password: "right",
            fail: false,
        }
    }

    #[test]
    fn unauthorized_response_has_challenge() {
        let response = OpdsAuthRejection::Unauthorized.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(response.headers().get(header::WWW_AUTHENTICATE).unwrap(), OPDS_CHALLENGE);
    }

    #[test]
    fn forbidden_and_internal_have_no_challenge() {
        let forbidden = OpdsAuthRejection::Forbidden.into_response();
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
        assert!(forbidden.headers().get(header::WWW_AUTHENTICATE).is_none());

        let internal = OpdsAuthRejection::Internal.into_response();
        assert_eq!(internal.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(internal.headers().get(header::WWW_AUTHENTICATE).is_none());
    }

    #[test]
    fn parse_basic_missing_header() {
        assert_eq!(parse_basic(&HeaderMap::new()), Err(OpdsAuthRejection::Unauthorized));
    }

    #[test]
    fn parse_basic_rejects_non_basic_scheme() {
        assert_eq!(parse_basic(&raw_auth("Bearer abc")), Err(OpdsAuthRejection::Unauthorized));
    }

    #[test]
    fn parse_basic_rejects_bad_base64() {
        assert_eq!(parse_basic(&raw_auth("Basic !!!")), Err(OpdsAuthRejection::Unauthorized));
    }

    #[test]
    fn parse_basic_rejects_missing_colon() {
        let value = format!("Basic {}", STANDARD.encode("alice"));
        assert_eq!(parse_basic(&raw_auth(&value)), Err(OpdsAuthRejection::Unauthorized));
    }

    #[test]
    fn parse_basic_rejects_empty_parts() {
        assert_eq!(parse_basic(&basic("", "pw")), Err(OpdsAuthRejection::Unauthorized));
        assert_eq!(parse_basic(&basic("alice", "")), Err(OpdsAuthRejection::Unauthorized));
    }

    #[test]
    fn parse_basic_password_with_colon() {
        assert_eq!(parse_basic(&basic("alice", "pa:ss")), Ok(("alice".to_owned(), "pa:ss".to_owned())));
    }

    #[test]
    fn parse_basic_scheme_case_insensitive() {
        let value = format!("basic {}", STANDARD.encode("alice:pw"));
        assert_eq!(parse_basic(&raw_auth(&value)), Ok(("alice".to_owned(), "pw".to_owned())));
    }

    #[tokio::test]
    async fn unknown_user_is_unauthorized() {
        let result = authenticate(&basic("alice", "right"), &store(None)).await;
        assert_eq!(result.unwrap_err(), OpdsAuthRejection::Unauthorized);
    }

    #[tokio::test]
    async fn wrong_password_is_unauthorized() {
        let result = authenticate(&basic("alice", "wrong"), &store(Some(user_with_opds_access()))).await;
        assert_eq!(result.unwrap_err(), OpdsAuthRejection::Unauthorized);
    }

    #[tokio::test]
    async fn no_capability_wrong_password_is_unauthorized() {
        let result = authenticate(&basic("alice", "wrong"), &store(Some(user_without_opds_access()))).await;
        assert_eq!(result.unwrap_err(), OpdsAuthRejection::Unauthorized);
    }

    #[tokio::test]
    async fn no_capability_correct_password_is_forbidden() {
        let result = authenticate(&basic("alice", "right"), &store(Some(user_without_opds_access()))).await;
        assert_eq!(result.unwrap_err(), OpdsAuthRejection::Forbidden);
    }

    #[tokio::test]
    async fn store_error_is_internal() {
        let failing = FakeStore {
            user: Some(user_with_opds_access()),
            password: "right",
            fail: true,
        };
        let result = authenticate(&basic("alice", "right"), &failing).await;
        assert_eq!(result.unwrap_err(), OpdsAuthRejection::Internal);
    }

    #[tokio::test]
    async fn valid_credentials_return_user() {
        let user = authenticate(&basic("alice", "right"), &store(Some(user_with_opds_access()))).await.unwrap();
        assert_eq!(user.username, "alice");
    }
}
