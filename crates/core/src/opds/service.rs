use std::sync::Arc;

use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Generate, Key},
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use rand::RngExt;
use sha2::{Digest, Sha256};

use crate::{Error, repository::RepositoryService, types::Capability, user::User, with_read_only_transaction, with_transaction};

const OPDS_PASSWORD_KEY: &str = "opds_password";
const OPDS_PASSWORD_LENGTH: usize = 12;
const OPDS_PASSWORD_CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789";

#[async_trait::async_trait]
pub trait OpdsService: Send + Sync {
    /// Returns the plaintext OPDS password for a user if one exists, or `None`
    /// if no password has been set up yet. This is a read-only operation and
    /// never creates a password.
    async fn get_password(&self, user: &User) -> Result<Option<String>, Error>;

    /// Returns the plaintext OPDS password for a user, generating one if none
    /// exists. Always returns the plaintext password (decrypted from storage).
    async fn get_or_create_password(&self, user: &User) -> Result<String, Error>;

    /// Generates a new OPDS password, replacing any existing one.
    /// Returns the new plaintext password.
    async fn regenerate_password(&self, user: &User) -> Result<String, Error>;

    /// Verifies a plaintext password against the stored OPDS password for a
    /// user. Does not check `OpdsAccess`; callers are responsible for
    /// capability checks.
    async fn verify_password(&self, user: &User, password: &str) -> Result<bool, Error>;
}

pub(crate) struct OpdsServiceImpl {
    repository_service: Arc<RepositoryService>,
    cipher: Aes256Gcm,
}

impl OpdsServiceImpl {
    pub(crate) fn new(repository_service: Arc<RepositoryService>, encryption_secret: &str) -> Self {
        let hash = Sha256::digest(encryption_secret.as_bytes());
        let key = Key::<Aes256Gcm>::try_from(hash.as_slice()).expect("SHA-256 output is 32 bytes");
        let cipher = Aes256Gcm::new(&key);
        Self { repository_service, cipher }
    }
}

fn generate_opds_password() -> String {
    let mut rng = rand::rng();
    (0..OPDS_PASSWORD_LENGTH)
        .map(|_| {
            let idx = rng.random_range(0..OPDS_PASSWORD_CHARSET.len());
            OPDS_PASSWORD_CHARSET[idx] as char
        })
        .collect()
}

fn encrypt_password(cipher: &Aes256Gcm, plaintext: &str) -> Result<String, Error> {
    let nonce = Nonce::generate();
    let ciphertext = cipher.encrypt(&nonce, plaintext.as_bytes()).map_err(|e| Error::CryptoError(e.to_string()))?;

    // Store as base64: nonce (12 bytes) || ciphertext
    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(BASE64.encode(&combined))
}

fn decrypt_password(cipher: &Aes256Gcm, stored: &str) -> Result<String, Error> {
    let combined = BASE64.decode(stored).map_err(|e| Error::CryptoError(e.to_string()))?;

    if combined.len() < 12 {
        return Err(Error::CryptoError("Invalid encrypted data".to_string()));
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::try_from(nonce_bytes).map_err(|e| Error::CryptoError(e.to_string()))?;
    let plaintext = cipher.decrypt(&nonce, ciphertext).map_err(|e| Error::CryptoError(e.to_string()))?;

    String::from_utf8(plaintext).map_err(|e| Error::CryptoError(e.to_string()))
}

#[async_trait::async_trait]
impl OpdsService for OpdsServiceImpl {
    async fn get_password(&self, user: &User) -> Result<Option<String>, Error> {
        if !user.has_capability(Capability::OpdsAccess) {
            return Err(Error::Validation("User does not have OPDS access".to_string()));
        }

        let user_id = user.id;
        let existing = with_read_only_transaction!(self, user_setting_repository, |tx| user_setting_repository
            .get(tx, user_id, OPDS_PASSWORD_KEY)
            .await)?;

        match existing {
            Some(setting) => decrypt_password(&self.cipher, &setting.value).map(Some),
            None => Ok(None),
        }
    }

    async fn get_or_create_password(&self, user: &User) -> Result<String, Error> {
        if !user.has_capability(Capability::OpdsAccess) {
            return Err(Error::Validation("User does not have OPDS access".to_string()));
        }

        let user_id = user.id;
        let existing = with_read_only_transaction!(self, user_setting_repository, |tx| user_setting_repository
            .get(tx, user_id, OPDS_PASSWORD_KEY)
            .await)?;

        if let Some(setting) = existing {
            return decrypt_password(&self.cipher, &setting.value);
        }

        let plaintext = generate_opds_password();
        let encrypted = encrypt_password(&self.cipher, &plaintext)?;

        let setting = crate::user::NewUserSetting {
            user_id,
            key: OPDS_PASSWORD_KEY.to_owned(),
            value: encrypted,
        };
        with_transaction!(self, user_setting_repository, |tx| user_setting_repository.set(tx, setting).await)?;

        Ok(plaintext)
    }

    async fn regenerate_password(&self, user: &User) -> Result<String, Error> {
        if !user.has_capability(Capability::OpdsAccess) {
            return Err(Error::Validation("User does not have OPDS access".to_string()));
        }

        let plaintext = generate_opds_password();
        let encrypted = encrypt_password(&self.cipher, &plaintext)?;

        let setting = crate::user::NewUserSetting {
            user_id: user.id,
            key: OPDS_PASSWORD_KEY.to_owned(),
            value: encrypted,
        };
        with_transaction!(self, user_setting_repository, |tx| user_setting_repository.set(tx, setting).await)?;

        Ok(plaintext)
    }

    async fn verify_password(&self, user: &User, password: &str) -> Result<bool, Error> {
        let user_id = user.id;
        let setting = with_read_only_transaction!(self, user_setting_repository, |tx| user_setting_repository
            .get(tx, user_id, OPDS_PASSWORD_KEY)
            .await)?;

        match setting {
            Some(s) => {
                let stored_plaintext = decrypt_password(&self.cipher, &s.value)?;
                Ok(stored_plaintext == password)
            }
            None => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::Utc;

    use super::*;
    use crate::{
        types::EmailAddress,
        user::{UserSetting, UserToken, repository::user_settings::MockUserSettingRepository},
    };

    // ── Unit tests ──────────────────────────────────────────────────────────

    const TEST_SECRET: &str = "test-encryption-secret";

    fn test_cipher() -> Aes256Gcm {
        let hash = Sha256::digest(TEST_SECRET.as_bytes());
        let key = Key::<Aes256Gcm>::try_from(hash.as_slice()).expect("SHA-256 output is 32 bytes");
        Aes256Gcm::new(&key)
    }

    #[test]
    fn test_generate_opds_password_length() {
        let password = generate_opds_password();
        assert_eq!(password.len(), OPDS_PASSWORD_LENGTH);
    }

    #[test]
    fn test_generate_opds_password_is_alphanumeric() {
        let password = generate_opds_password();
        assert!(password.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_generate_opds_password_uniqueness() {
        let p1 = generate_opds_password();
        let p2 = generate_opds_password();
        assert_ne!(p1, p2);
    }

    #[test]
    fn test_encrypt_decrypt_round_trip() {
        let cipher = test_cipher();
        let plaintext = "testpassword";
        let encrypted = encrypt_password(&cipher, plaintext).unwrap();
        let decrypted = decrypt_password(&cipher, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_produces_different_ciphertexts() {
        let cipher = test_cipher();
        let e1 = encrypt_password(&cipher, "same").unwrap();
        let e2 = encrypt_password(&cipher, "same").unwrap();
        assert_ne!(e1, e2, "different nonces should produce different ciphertexts");
    }

    #[test]
    fn test_decrypt_invalid_data() {
        let cipher = test_cipher();
        decrypt_password(&cipher, "not-valid-base64!!!").unwrap_err();
    }

    #[test]
    fn test_decrypt_wrong_key() {
        let cipher = test_cipher();
        let encrypted = encrypt_password(&cipher, "secret").unwrap();

        let wrong_hash = Sha256::digest(b"wrong-secret");
        let wrong_key = Key::<Aes256Gcm>::try_from(wrong_hash.as_slice()).expect("SHA-256 output is 32 bytes");
        let wrong_cipher = Aes256Gcm::new(&wrong_key);
        decrypt_password(&wrong_cipher, &encrypted).unwrap_err();
    }

    // ── Helpers ─────────────────────────────────────────────────────────────

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

    fn create_service(mock_settings: MockUserSettingRepository) -> OpdsServiceImpl {
        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .user_setting_repository(Arc::new(mock_settings))
                .build()
                .expect("all fields provided"),
        );
        OpdsServiceImpl::new(repository_service, TEST_SECRET)
    }

    // ── get_or_create_password ──────────────────────────────────────────────

    #[tokio::test]
    async fn get_or_create_password_creates_when_none_exists() {
        let mut mock = MockUserSettingRepository::new();
        mock.expect_get().returning(|_, _, _| Box::pin(async { Ok(None) }));
        mock.expect_set().returning(|_, _| {
            Box::pin(async {
                Ok(UserSetting {
                    user_id: 1,
                    key: OPDS_PASSWORD_KEY.to_owned(),
                    value: "encrypted-placeholder".to_owned(),
                })
            })
        });
        let svc = create_service(mock);

        let pw = svc.get_or_create_password(&user_with_opds_access()).await.unwrap();
        assert_eq!(pw.len(), OPDS_PASSWORD_LENGTH);
        assert!(pw.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[tokio::test]
    async fn get_or_create_password_returns_existing_decrypted() {
        let cipher = test_cipher();
        let encrypted = encrypt_password(&cipher, "existing-pw").unwrap();

        let mut mock = MockUserSettingRepository::new();
        mock.expect_get().returning(move |_, _, _| {
            let encrypted = encrypted.clone();
            Box::pin(async move {
                Ok(Some(UserSetting {
                    user_id: 1,
                    key: OPDS_PASSWORD_KEY.to_owned(),
                    value: encrypted,
                }))
            })
        });
        let svc = create_service(mock);

        let pw = svc.get_or_create_password(&user_with_opds_access()).await.unwrap();
        assert_eq!(pw, "existing-pw");
    }

    #[tokio::test]
    async fn get_or_create_password_rejects_user_without_capability() {
        let svc = create_service(MockUserSettingRepository::new());

        let result = svc.get_or_create_password(&user_without_opds_access()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("OPDS access"));
    }

    // ── regenerate_password ─────────────────────────────────────────────────

    #[tokio::test]
    async fn regenerate_password_returns_new_password() {
        let mut mock = MockUserSettingRepository::new();
        mock.expect_set().returning(|_, _| {
            Box::pin(async {
                Ok(UserSetting {
                    user_id: 1,
                    key: OPDS_PASSWORD_KEY.to_owned(),
                    value: "encrypted-placeholder".to_owned(),
                })
            })
        });
        let svc = create_service(mock);

        let pw = svc.regenerate_password(&user_with_opds_access()).await.unwrap();
        assert_eq!(pw.len(), OPDS_PASSWORD_LENGTH);
        assert!(pw.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[tokio::test]
    async fn regenerate_password_rejects_user_without_capability() {
        let svc = create_service(MockUserSettingRepository::new());

        let result = svc.regenerate_password(&user_without_opds_access()).await;
        result.unwrap_err();
    }

    // ── verify_password ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn verify_password_returns_true_for_correct_password() {
        let cipher = test_cipher();
        let encrypted = encrypt_password(&cipher, "testpassword").unwrap();

        let mut mock = MockUserSettingRepository::new();
        mock.expect_get().returning(move |_, _, _| {
            let encrypted = encrypted.clone();
            Box::pin(async move {
                Ok(Some(UserSetting {
                    user_id: 1,
                    key: OPDS_PASSWORD_KEY.to_owned(),
                    value: encrypted,
                }))
            })
        });
        let svc = create_service(mock);

        let valid = svc.verify_password(&user_with_opds_access(), "testpassword").await.unwrap();
        assert!(valid);
    }

    #[tokio::test]
    async fn verify_password_returns_false_for_wrong_password() {
        let cipher = test_cipher();
        let encrypted = encrypt_password(&cipher, "correct").unwrap();

        let mut mock = MockUserSettingRepository::new();
        mock.expect_get().returning(move |_, _, _| {
            let encrypted = encrypted.clone();
            Box::pin(async move {
                Ok(Some(UserSetting {
                    user_id: 1,
                    key: OPDS_PASSWORD_KEY.to_owned(),
                    value: encrypted,
                }))
            })
        });
        let svc = create_service(mock);

        let valid = svc.verify_password(&user_with_opds_access(), "wrong").await.unwrap();
        assert!(!valid);
    }

    #[tokio::test]
    async fn verify_password_returns_false_when_no_password_stored() {
        let mut mock = MockUserSettingRepository::new();
        mock.expect_get().returning(|_, _, _| Box::pin(async { Ok(None) }));
        let svc = create_service(mock);

        let valid = svc.verify_password(&user_with_opds_access(), "anything").await.unwrap();
        assert!(!valid);
    }

    #[tokio::test]
    async fn verify_password_ignores_capability() {
        let cipher = test_cipher();
        let encrypted = encrypt_password(&cipher, "testpassword").unwrap();

        let mut mock = MockUserSettingRepository::new();
        mock.expect_get().returning(move |_, _, _| {
            let encrypted = encrypted.clone();
            Box::pin(async move {
                Ok(Some(UserSetting {
                    user_id: 1,
                    key: OPDS_PASSWORD_KEY.to_owned(),
                    value: encrypted,
                }))
            })
        });
        let svc = create_service(mock);

        let valid = svc.verify_password(&user_without_opds_access(), "testpassword").await.unwrap();
        assert!(valid);
    }
}
