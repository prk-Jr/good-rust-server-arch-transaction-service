//! Security utilities for API key hashing and webhook signing.

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Hashes an API key using SHA-256.
pub fn hash_api_key(key: &str) -> String {
    let hash = Sha256::digest(key.as_bytes());
    hex::encode(hash)
}

/// Verifies an API key against a stored hash using constant-time comparison.
pub fn verify_api_key(input: &str, stored_hash: &str) -> bool {
    let input_hash = hash_api_key(input);
    input_hash.as_bytes().ct_eq(stored_hash.as_bytes()).into()
}

/// Signs a webhook payload using HMAC-SHA256.
pub fn sign_webhook(payload: &[u8], secret: &str) -> String {
    use hmac::{Hmac, Mac};

    type HmacSha256 = Hmac<Sha256>;

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(payload);
    hex::encode(mac.finalize().into_bytes())
}

/// Verifies a webhook signature using constant-time comparison.
pub fn verify_webhook_signature(payload: &[u8], signature: &str, secret: &str) -> bool {
    let expected = sign_webhook(payload, secret);
    expected.as_bytes().ct_eq(signature.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_hashing() {
        let key = "sk_test_abc123";
        let hash = hash_api_key(key);

        assert_eq!(hash.len(), 64);
        assert_eq!(hash, hash_api_key(key));
    }

    #[test]
    fn test_api_key_verification() {
        let key = "sk_test_abc123";
        let hash = hash_api_key(key);

        assert!(verify_api_key(key, &hash));
        assert!(!verify_api_key("wrong_key", &hash));
    }

    #[test]
    fn test_webhook_signing() {
        let payload = br#"{"event":"transaction.created"}"#;
        let secret = "webhook_secret_123";

        let signature = sign_webhook(payload, secret);
        assert!(verify_webhook_signature(payload, &signature, secret));
        assert!(!verify_webhook_signature(
            payload,
            &signature,
            "wrong_secret"
        ));
        assert!(!verify_webhook_signature(b"tampered", &signature, secret));
    }
}
