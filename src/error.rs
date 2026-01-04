//! Error types for tola-vdom.
//!
//! Provides user-friendly error types that hide internal implementation details.

use thiserror::Error;

/// Errors that can occur during VDOM operations.
#[derive(Debug, Error)]
pub enum VdomError {
    /// Cache version mismatch - data was serialized with an incompatible version
    #[error("cache version mismatch: expected v{expected}, found v{found}")]
    VersionMismatch {
        /// Expected schema version
        expected: u16,
        /// Found schema version
        found: u16,
    },

    /// Cache data is corrupted or invalid
    #[error("cache corrupted: {0}")]
    Corrupted(String),

    /// Serialization/deserialization failed
    #[error("serialization error: {0}")]
    Serialize(String),

    /// Magic bytes validation failed
    #[error("invalid cache format: expected magic bytes {expected:?}, found {found:?}")]
    InvalidMagic {
        /// Expected magic bytes
        expected: [u8; 8],
        /// Found magic bytes
        found: [u8; 8],
    },
}

/// Result type alias for VDOM operations.
pub type VdomResult<T> = Result<T, VdomError>;

impl VdomError {
    /// Create a corruption error with a message.
    pub fn corrupted(msg: impl Into<String>) -> Self {
        Self::Corrupted(msg.into())
    }

    /// Create a serialization error from any error type.
    pub fn serialize(err: impl std::error::Error) -> Self {
        Self::Serialize(err.to_string())
    }
}

#[cfg(feature = "cache")]
impl From<rkyv::rancor::Error> for VdomError {
    fn from(err: rkyv::rancor::Error) -> Self {
        Self::Serialize(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = VdomError::VersionMismatch { expected: 2, found: 1 };
        assert_eq!(err.to_string(), "cache version mismatch: expected v2, found v1");

        let err = VdomError::Corrupted("bad data".to_string());
        assert_eq!(err.to_string(), "cache corrupted: bad data");
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<VdomError>();
    }
}
