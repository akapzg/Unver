use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

use crate::errors::{AppError, AppResult};

/// Claims embedded in the JWT access token
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id
    pub exp: usize,  // expiry (Unix timestamp)
    pub iat: usize,  // issued at
}

/// Hash a password with Argon2id
pub fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Hash failed: {e}")))
}

/// Verify a password against an Argon2 hash
pub fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Invalid hash: {e}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Validate password strength (min 8 chars)
pub fn validate_password(password: &str) -> AppResult<()> {
    if password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }
    Ok(())
}

/// Validate username (3-32 chars, alphanumeric + underscore)
pub fn validate_username(username: &str) -> AppResult<()> {
    if username.len() < 3 || username.len() > 32 {
        return Err(AppError::BadRequest(
            "Username must be 3-32 characters".to_string(),
        ));
    }
    if !username.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(AppError::BadRequest(
            "Username may only contain letters, digits, and underscores".to_string(),
        ));
    }
    Ok(())
}

/// Issue a JWT access token (1-hour expiry)
pub fn issue_access_token(user_id: &str, secret: &str) -> AppResult<String> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id.to_string(),
        iat: now.timestamp() as usize,
        exp: (now + Duration::hours(1)).timestamp() as usize,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!("JWT encode: {e}")))
}

/// Validate a JWT access token and return the user_id
pub fn validate_access_token(token: &str, secret: &str) -> AppResult<String> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|_| AppError::Unauthorized)?;
    Ok(data.claims.sub)
}

/// Hash a refresh token for storage using SHA-256 (tokens are already random,
/// so a fast cryptographic hash is sufficient — no need for Argon2).
pub fn hash_token(token: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Verify a refresh token against its stored SHA-256 hash
pub fn verify_token_hash(token: &str, hash: &str) -> bool {
    use sha2::{Sha256, Digest};
    let digest = Sha256::digest(token.as_bytes());
    
    let expected = match hex::decode(hash) {
        Ok(v) if v.len() == 32 => v,
        _ => return false,
    };
    
    ring::constant_time::verify_slices_are_equal(digest.as_slice(), &expected).is_ok()
}

/// Generate a cryptographically random API key  
/// Format: `unver_<32 random chars>`
pub fn generate_api_key() -> String {
    use rand::Rng;
    let body: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    format!("unver_{body}")
}

/// Generate a refresh token (UUID v4)
pub fn generate_refresh_token() -> String {
    uuid::Uuid::new_v4().to_string()
}

// ── Data Encryption ────────────────────────────────────────────────────────

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};

/// Encrypt data using a master secret. Returns base64 string.
pub fn encrypt_data(data: &str, master_secret: &str) -> AppResult<String> {
    let key_bytes = derive_key(master_secret);
    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    
    let nonce_bytes = rand::random::<[u8; 12]>();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, data.as_bytes())
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Encryption failed")))?;

    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);
    
    Ok(STANDARD.encode(combined))
}

/// Decrypt data using a master secret.
pub fn decrypt_data(encrypted_b64: &str, master_secret: &str) -> AppResult<String> {
    let combined = STANDARD.decode(encrypted_b64)
        .map_err(|_| AppError::BadRequest("Invalid base64 in encrypted data".to_string()))?;
    
    if combined.len() < 12 {
        return Err(AppError::BadRequest("Invalid encrypted data format".to_string()));
    }

    let key_bytes = derive_key(master_secret);
    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Decryption failed - possibly wrong master key")))?;

    String::from_utf8(plaintext)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Decrypted data is not valid UTF-8")))
}

fn derive_key(secret: &str) -> [u8; 32] {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Password ────────────────────────────────────────────────────────

    #[test]
    fn test_password_hash_and_verify() {
        let hash = hash_password("MyP@ssw0rd!").unwrap();
        assert!(verify_password("MyP@ssw0rd!", &hash).unwrap());
        assert!(!verify_password("wrong", &hash).unwrap());
    }

    #[test]
    fn test_password_hash_is_random() {
        let h1 = hash_password("same").unwrap();
        let h2 = hash_password("same").unwrap();
        assert_ne!(h1, h2, "same password should produce different hashes");
        assert!(verify_password("same", &h1).unwrap());
        assert!(verify_password("same", &h2).unwrap());
    }

    #[test]
    fn test_validate_password_min_length() {
        assert!(validate_password("12345678").is_ok());
        assert!(validate_password("short").is_err());
    }

    #[test]
    fn test_validate_username() {
        assert!(validate_username("abc").is_ok());
        assert!(validate_username("user_name").is_ok());
        assert!(validate_username("ab").is_err());         // too short
        assert!(validate_username(&"x".repeat(33)).is_err()); // too long
        assert!(validate_username("user!").is_err());      // special char
    }

    // ── JWT ─────────────────────────────────────────────────────────────

    #[test]
    fn test_jwt_roundtrip() {
        let secret = "test-secret-key-32-bytes-xxxx";
        let token = issue_access_token("user-123", secret).unwrap();
        assert!(!token.is_empty());
        let user_id = validate_access_token(&token, secret).unwrap();
        assert_eq!(user_id, "user-123");
    }

    #[test]
    fn test_jwt_wrong_secret_rejected() {
        let token = issue_access_token("user", "secret-a").unwrap();
        assert!(validate_access_token(&token, "secret-b").is_err());
    }

    #[test]
    fn test_jwt_tampered_token_rejected() {
        let token = issue_access_token("user", "secret").unwrap();
        let mut parts: Vec<&str> = token.split('.').collect();
        parts[2] = "tampered"; // corrupt signature
        let tampered = parts.join(".");
        assert!(validate_access_token(&tampered, "secret").is_err());
    }

    // ── Token hash ──────────────────────────────────────────────────────

    #[test]
    fn test_token_hash_verify() {
        let token = "random-refresh-token-value";
        let hash = hash_token(token);
        assert!(verify_token_hash(token, &hash));
        assert!(!verify_token_hash("different", &hash));
    }

    #[test]
    fn test_token_hash_constant_time_rejects_invalid_hex() {
        assert!(!verify_token_hash("token", "not-valid-hex"));
        assert!(!verify_token_hash("token", "abcd")); // too short
    }

    // ── API key ─────────────────────────────────────────────────────────

    #[test]
    fn test_generate_api_key_format() {
        let key = generate_api_key();
        assert!(key.starts_with("unver_"));
        assert_eq!(key.len(), 6 + 32); // "unver_" + 32 alphanumeric
    }

    #[test]
    fn test_generate_api_key_unique() {
        let k1 = generate_api_key();
        let k2 = generate_api_key();
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_generate_refresh_token() {
        let t1 = generate_refresh_token();
        let t2 = generate_refresh_token();
        assert!(!t1.is_empty());
        assert_ne!(t1, t2);
    }

    // ── AES-256-GCM encryption ──────────────────────────────────────────

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let secret = "my-master-secret-key";
        let original = "-----BEGIN PRIVATE KEY-----\nsensitive key data\n-----END PRIVATE KEY-----";
        let encrypted = encrypt_data(original, secret).unwrap();
        // Encrypted value should be different from original
        assert_ne!(encrypted, original);
        // Should be valid base64
        assert!(base64::engine::general_purpose::STANDARD
            .decode(&encrypted).is_ok());
        let decrypted = decrypt_data(&encrypted, secret).unwrap();
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_encrypt_same_input_different_output() {
        let secret = "master-secret";
        let e1 = encrypt_data("hello", secret).unwrap();
        let e2 = encrypt_data("hello", secret).unwrap();
        assert_ne!(e1, e2, "same plaintext should yield different ciphertext (random nonce)");
        assert_eq!(decrypt_data(&e1, secret).unwrap(), "hello");
        assert_eq!(decrypt_data(&e2, secret).unwrap(), "hello");
    }

    #[test]
    fn test_decrypt_wrong_secret_fails() {
        let encrypted = encrypt_data("data", "correct-key").unwrap();
        assert!(decrypt_data(&encrypted, "wrong-key").is_err());
    }

    #[test]
    fn test_decrypt_invalid_base64() {
        assert!(decrypt_data("!!!not base64!!!", "key").is_err());
    }

    #[test]
    fn test_decrypt_too_short() {
        let short = base64::engine::general_purpose::STANDARD.encode(&[1, 2, 3]);
        assert!(decrypt_data(&short, "key").is_err());
    }

    #[test]
    fn test_encrypt_empty_string() {
        let encrypted = encrypt_data("", "secret").unwrap();
        let decrypted = decrypt_data(&encrypted, "secret").unwrap();
        assert_eq!(decrypted, "");
    }

    #[test]
    fn test_encrypt_unicode() {
        let original = "证书密钥 🔐 test";
        let encrypted = encrypt_data(original, "密钥").unwrap();
        let decrypted = decrypt_data(&encrypted, "密钥").unwrap();
        assert_eq!(decrypted, original);
    }
}
