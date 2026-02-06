//! # blurhash-core
//!
//! Fast BlurHash encoding and decoding in pure Rust.
//!
//! [BlurHash](https://blurha.sh/) is a compact representation of a placeholder
//! for an image. This crate provides high-performance encoding and decoding
//! with precomputed lookup tables and cache-friendly memory access patterns.
//!
//! ## Quick Start
//!
//! ```
//! use blurhash_core::{encode, decode};
//!
//! // Encode: image pixels -> BlurHash string
//! let pixels = vec![128u8; 4 * 4 * 3]; // 4x4 gray image
//! let hash = encode(&pixels, 4, 4, 4, 3).unwrap();
//!
//! // Decode: BlurHash string -> image pixels
//! let decoded = decode(&hash, 32, 32, 1.0).unwrap();
//! assert_eq!(decoded.len(), 32 * 32 * 3);
//! ```

pub mod base83;
pub mod color;
pub mod error;

mod decode_impl;
mod encode_impl;

#[cfg(feature = "simd")]
#[allow(dead_code)]
pub(crate) mod simd;

#[cfg(feature = "gpu")]
pub mod gpu;

// Re-export primary functions at crate root.
pub use color::{
    linear_to_srgb, linear_to_srgb_f32, sign_pow, sign_pow_f32, srgb_to_linear,
    srgb_to_linear_f32,
};
pub use decode_impl::{components, decode};
pub use encode_impl::encode;
pub use error::BlurhashError;
