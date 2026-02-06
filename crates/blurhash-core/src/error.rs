//! Error types for BlurHash encoding and decoding.

use thiserror::Error;

/// Errors that can occur during BlurHash encoding or decoding.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BlurhashError {
    /// The BlurHash string has an invalid length.
    #[error("invalid BlurHash length: expected {expected}, got {actual}")]
    InvalidLength {
        /// The expected length.
        expected: usize,
        /// The actual length.
        actual: usize,
    },

    /// The component count is out of the valid range (1..=9).
    #[error("component count out of range: {component} = {value} (must be 1..=9)")]
    InvalidComponentCount {
        /// Which component axis ("x" or "y").
        component: &'static str,
        /// The invalid value.
        value: u32,
    },

    /// An invalid character was encountered during base83 decoding.
    #[error("invalid base83 character: {0:?}")]
    InvalidBase83Character(char),

    /// A general encoding error.
    #[error("encoding error: {0}")]
    EncodingError(String),

    /// The image dimensions are invalid (zero or too large).
    #[error("invalid dimensions: {width}x{height} ({reason})")]
    InvalidDimensions {
        /// The width value.
        width: u32,
        /// The height value.
        height: u32,
        /// Why the dimensions are invalid.
        reason: &'static str,
    },
}
