//! Content hash computation for change detection.

use sha2::{Digest, Sha256};
use std::fmt;
use thiserror::Error;

/// SHA256 hash of file content for change detection.
///
/// Stores a 64-character lowercase hex string representing the hash.
/// Computed from raw file bytes (before BOM stripping or encoding conversion)
/// to capture the exact file state on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentHash {
    hex: String,
}

/// Errors when parsing a content hash from a hex string.
#[derive(Debug, Error)]
pub enum ContentHashError {
    #[error("invalid hex string: expected 64 lowercase hex characters, got {0} characters")]
    InvalidLength(usize),

    #[error("invalid hex character at position {position}: '{character}'")]
    InvalidCharacter { position: usize, character: char },
}

impl ContentHash {
    /// Computes a SHA256 hash of the given bytes.
    ///
    /// The hash is returned as a 64-character lowercase hex string.
    pub fn compute(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let result = hasher.finalize();
        let hex = format!("{:x}", result);
        Self { hex }
    }

    /// Creates a ContentHash from a hex string.
    ///
    /// The input must be exactly 64 hex characters. It will be normalized
    /// to lowercase.
    ///
    /// # Errors
    ///
    /// Returns `ContentHashError::InvalidLength` if the string is not 64 characters.
    /// Returns `ContentHashError::InvalidCharacter` if the string contains non-hex characters.
    pub fn from_hex(hex: &str) -> Result<Self, ContentHashError> {
        if hex.len() != 64 {
            return Err(ContentHashError::InvalidLength(hex.len()));
        }

        for (i, c) in hex.chars().enumerate() {
            if !c.is_ascii_hexdigit() {
                return Err(ContentHashError::InvalidCharacter {
                    position: i,
                    character: c,
                });
            }
        }

        Ok(Self {
            hex: hex.to_ascii_lowercase(),
        })
    }

    /// Returns the hash as a 64-character lowercase hex string.
    pub fn as_str(&self) -> &str {
        &self.hex
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.hex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===========================================
    // Phase 1: ContentHash Type
    // ===========================================

    // --- Cycle 1.1: Basic Creation ---

    #[test]
    fn content_hash_from_empty_bytes() {
        let hash = ContentHash::compute(&[]);
        assert_eq!(
            hash.as_str(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    // --- Cycle 1.2: Known Content ---

    #[test]
    fn content_hash_from_known_content() {
        let hash = ContentHash::compute(b"hello world");
        assert_eq!(
            hash.as_str(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    // --- Cycle 1.3: Display/Debug ---

    #[test]
    fn content_hash_display_shows_hex_string() {
        let hash = ContentHash::compute(b"test");
        assert_eq!(format!("{}", hash).len(), 64);
    }

    #[test]
    fn content_hash_debug_includes_hex() {
        let hash = ContentHash::compute(b"test");
        let debug = format!("{:?}", hash);
        assert!(debug.contains("ContentHash"));
        assert!(debug.contains(&hash.as_str()[..16])); // At least part of the hash
    }

    // --- Cycle 1.4: Equality ---

    #[test]
    fn content_hash_equality_same_content() {
        let hash1 = ContentHash::compute(b"same");
        let hash2 = ContentHash::compute(b"same");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn content_hash_inequality_different_content() {
        let hash1 = ContentHash::compute(b"first");
        let hash2 = ContentHash::compute(b"second");
        assert_ne!(hash1, hash2);
    }

    // --- Cycle 1.5: From Hex String ---

    #[test]
    fn content_hash_from_hex_string() {
        let hex = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let hash = ContentHash::from_hex(hex).unwrap();
        assert_eq!(hash.as_str(), hex);
    }

    #[test]
    fn content_hash_from_invalid_hex_fails_wrong_length() {
        assert!(ContentHash::from_hex("invalid").is_err());
        assert!(ContentHash::from_hex("abcd1234").is_err()); // wrong length
    }

    #[test]
    fn content_hash_from_invalid_hex_fails_invalid_chars() {
        // 64 characters but with invalid hex chars
        let invalid = "g3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let result = ContentHash::from_hex(invalid);
        assert!(matches!(
            result,
            Err(ContentHashError::InvalidCharacter { .. })
        ));
    }

    #[test]
    fn content_hash_from_empty_string_fails() {
        let result = ContentHash::from_hex("");
        assert!(matches!(result, Err(ContentHashError::InvalidLength(0))));
    }

    // --- Cycle 1.6: Normalizes to Lowercase ---

    #[test]
    fn content_hash_from_hex_normalizes_to_lowercase() {
        let uppercase = "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855";
        let hash = ContentHash::from_hex(uppercase).unwrap();
        assert!(hash.as_str().chars().all(|c| !c.is_ascii_uppercase()));
        assert_eq!(
            hash.as_str(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn content_hash_from_hex_mixed_case_normalizes() {
        let mixed = "E3b0C44298fc1C149AFBF4c8996fb92427ae41E4649b934cA495991b7852B855";
        let hash = ContentHash::from_hex(mixed).unwrap();
        assert!(hash.as_str().chars().all(|c| !c.is_ascii_uppercase()));
    }

    // --- Additional Edge Cases ---

    #[test]
    fn content_hash_clone_works() {
        let hash1 = ContentHash::compute(b"test");
        let hash2 = hash1.clone();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn content_hash_roundtrip_compute_to_from_hex() {
        let original = ContentHash::compute(b"some content");
        let from_hex = ContentHash::from_hex(original.as_str()).unwrap();
        assert_eq!(original, from_hex);
    }
}
