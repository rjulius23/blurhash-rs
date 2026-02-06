//! BlurHash decoding: convert a BlurHash string back into an RGB image.
//!
//! The decoder parses the base83-encoded BlurHash string, extracts the DCT
//! components, and reconstructs an image of the specified dimensions.

use std::f64::consts::PI;

use crate::base83;
use crate::color::{linear_to_srgb, sign_pow, srgb_to_linear};
use crate::error::BlurhashError;

/// Extract the number of X and Y components from a BlurHash string.
///
/// # Errors
///
/// Returns [`BlurhashError::InvalidLength`] if the BlurHash is too short (< 6 characters).
///
/// # Examples
///
/// ```
/// use blurhash_core::components;
/// // A valid BlurHash for 4x3 components
/// let (cx, cy) = components("LEHV6nWB2yk8pyo0adR*.7kCMdnj").unwrap();
/// assert!(cx >= 1 && cx <= 9);
/// assert!(cy >= 1 && cy <= 9);
/// ```
pub fn components(blurhash: &str) -> Result<(u32, u32), BlurhashError> {
    if blurhash.len() < 6 {
        return Err(BlurhashError::InvalidLength {
            expected: 6,
            actual: blurhash.len(),
        });
    }
    let size_info = base83::decode(&blurhash[0..1])?;
    let size_y = (size_info / 9) + 1;
    let size_x = (size_info % 9) + 1;
    Ok((size_x as u32, size_y as u32))
}

/// Decode a BlurHash string into a flat RGB byte array.
///
/// # Arguments
///
/// * `blurhash` - The BlurHash string to decode.
/// * `width` - The desired output image width.
/// * `height` - The desired output image height.
/// * `punch` - Factor to boost/reduce contrast of the decoded image (1.0 = normal).
///
/// # Returns
///
/// A `Vec<u8>` of length `width * height * 3` containing RGB pixel data in
/// row-major order.
///
/// # Errors
///
/// Returns an error if the BlurHash string is invalid (wrong length or invalid characters).
///
/// # Examples
///
/// ```
/// use blurhash_core::decode;
/// let pixels = decode("LEHV6nWB2yk8pyo0adR*.7kCMdnj", 32, 32, 1.0).unwrap();
/// assert_eq!(pixels.len(), 32 * 32 * 3);
/// ```
pub fn decode(
    blurhash: &str,
    width: u32,
    height: u32,
    punch: f64,
) -> Result<Vec<u8>, BlurhashError> {
    if blurhash.len() < 6 {
        return Err(BlurhashError::InvalidLength {
            expected: 6,
            actual: blurhash.len(),
        });
    }

    let size_info = base83::decode(&blurhash[0..1])?;
    let size_y = (size_info / 9) + 1;
    let size_x = (size_info % 9) + 1;

    let expected_len = 4 + 2 * (size_x * size_y) as usize;
    if blurhash.len() != expected_len {
        return Err(BlurhashError::InvalidLength {
            expected: expected_len,
            actual: blurhash.len(),
        });
    }

    let quant_max_value = base83::decode(&blurhash[1..2])?;
    let real_max_value = (quant_max_value as f64 + 1.0) / 166.0 * punch;

    // Decode DC component.
    let dc_value = base83::decode(&blurhash[2..6])?;
    let dc_r = srgb_to_linear((dc_value >> 16) as u8);
    let dc_g = srgb_to_linear(((dc_value >> 8) & 255) as u8);
    let dc_b = srgb_to_linear((dc_value & 255) as u8);

    let num_components = (size_x * size_y) as usize;
    let mut colours: Vec<[f64; 3]> = Vec::with_capacity(num_components);
    colours.push([dc_r, dc_g, dc_b]);

    // Decode AC components.
    for component_idx in 1..num_components {
        let start = 4 + component_idx * 2;
        let ac_value = base83::decode(&blurhash[start..start + 2])?;

        let quant_r = (ac_value / (19 * 19)) as f64;
        let quant_g = ((ac_value / 19) % 19) as f64;
        let quant_b = (ac_value % 19) as f64;

        colours.push([
            sign_pow((quant_r - 9.0) / 9.0, 2.0) * real_max_value,
            sign_pow((quant_g - 9.0) / 9.0, 2.0) * real_max_value,
            sign_pow((quant_b - 9.0) / 9.0, 2.0) * real_max_value,
        ]);
    }

    let w = width as usize;
    let h = height as usize;
    let wf = width as f64;
    let hf = height as f64;

    // Precompute cosine tables.
    let cos_x: Vec<Vec<f64>> = (0..size_x as usize)
        .map(|i| {
            (0..w)
                .map(|x| (PI * x as f64 * i as f64 / wf).cos())
                .collect()
        })
        .collect();
    let cos_y: Vec<Vec<f64>> = (0..size_y as usize)
        .map(|j| {
            (0..h)
                .map(|y| (PI * y as f64 * j as f64 / hf).cos())
                .collect()
        })
        .collect();

    // Reconstruct the image.
    let mut result = vec![0u8; w * h * 3];

    for y in 0..h {
        for x in 0..w {
            let mut pixel_r = 0.0f64;
            let mut pixel_g = 0.0f64;
            let mut pixel_b = 0.0f64;

            for j in 0..size_y as usize {
                let cy = cos_y[j][y];
                for i in 0..size_x as usize {
                    let basis = cos_x[i][x] * cy;
                    let colour = &colours[i + j * size_x as usize];
                    pixel_r += colour[0] * basis;
                    pixel_g += colour[1] * basis;
                    pixel_b += colour[2] * basis;
                }
            }

            let idx = (y * w + x) * 3;
            result[idx] = linear_to_srgb(pixel_r);
            result[idx + 1] = linear_to_srgb(pixel_g);
            result[idx + 2] = linear_to_srgb(pixel_b);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode_impl;

    #[test]
    fn test_components_extraction() {
        // Size flag 21 = (4-1) + (3-1)*9 = 3 + 18
        // That encodes as base83(21) = "L"
        // Build a minimal valid blurhash with 4x3 components
        let hash = "LEHV6nWB2yk8pyo0adR*.7kCMdnj";
        let (cx, cy) = components(hash).unwrap();
        assert_eq!(cx, 4);
        assert_eq!(cy, 3);
    }

    #[test]
    fn test_components_too_short() {
        assert!(components("ABC").is_err());
    }

    #[test]
    fn test_decode_output_size() {
        let hash = "LEHV6nWB2yk8pyo0adR*.7kCMdnj";
        let pixels = decode(hash, 32, 32, 1.0).unwrap();
        assert_eq!(pixels.len(), 32 * 32 * 3);
    }

    #[test]
    fn test_decode_too_short() {
        assert!(decode("ABC", 32, 32, 1.0).is_err());
    }

    #[test]
    fn test_decode_wrong_length() {
        // Valid first char but string is truncated
        assert!(decode("L00000", 32, 32, 1.0).is_err());
    }

    #[test]
    fn test_encode_decode_roundtrip_solid() {
        // Solid gray image with 1x1 components (DC-only) for exact roundtrip
        let pixels = vec![128u8; 4 * 4 * 3];
        let hash = encode_impl::encode(&pixels, 4, 4, 1, 1).unwrap();
        let decoded = decode(&hash, 4, 4, 1.0).unwrap();

        // With 1x1 components (DC-only), all pixels should be very close to original
        for i in 0..16 {
            let r = decoded[i * 3];
            let g = decoded[i * 3 + 1];
            let b = decoded[i * 3 + 2];
            assert!(
                (r as i16 - 128).unsigned_abs() <= 1,
                "pixel {i} R: expected ~128, got {r}"
            );
            assert!(
                (g as i16 - 128).unsigned_abs() <= 1,
                "pixel {i} G: expected ~128, got {g}"
            );
            assert!(
                (b as i16 - 128).unsigned_abs() <= 1,
                "pixel {i} B: expected ~128, got {b}"
            );
        }
    }

    #[test]
    fn test_encode_decode_roundtrip_color() {
        // A 4x4 image with some color variation
        let mut pixels = vec![0u8; 4 * 4 * 3];
        for y in 0..4 {
            for x in 0..4 {
                let idx = (y * 4 + x) * 3;
                pixels[idx] = (x * 64) as u8;
                pixels[idx + 1] = (y * 64) as u8;
                pixels[idx + 2] = 128;
            }
        }
        let hash = encode_impl::encode(&pixels, 4, 4, 4, 3).unwrap();
        let decoded = decode(&hash, 4, 4, 1.0).unwrap();
        assert_eq!(decoded.len(), 4 * 4 * 3);
    }

    #[test]
    fn test_decode_known_hash() {
        // "LEHV6nWB2yk8pyo0adR*.7kCMdnj" is a well-known BlurHash
        let pixels = decode("LEHV6nWB2yk8pyo0adR*.7kCMdnj", 4, 4, 1.0).unwrap();
        assert_eq!(pixels.len(), 4 * 4 * 3);
        // Verify pixels are in valid range
        for &p in &pixels {
            assert!(p <= 255);
        }
    }

    #[test]
    fn test_decode_punch() {
        let hash = "LEHV6nWB2yk8pyo0adR*.7kCMdnj";
        let normal = decode(hash, 4, 4, 1.0).unwrap();
        let punched = decode(hash, 4, 4, 2.0).unwrap();
        // Punched version should generally have more contrast
        // (different pixel values from normal)
        assert_ne!(normal, punched);
    }

    #[test]
    fn test_decode_1x1() {
        // Minimal blurhash: 1x1 components, length = 4 + 2*1 = 6
        let pixels = vec![200u8; 2 * 2 * 3];
        let hash = encode_impl::encode(&pixels, 2, 2, 1, 1).unwrap();
        let decoded = decode(&hash, 4, 4, 1.0).unwrap();
        // With 1x1 components, all output pixels should be the same
        let first = [decoded[0], decoded[1], decoded[2]];
        for i in 1..16 {
            assert_eq!(
                [decoded[i * 3], decoded[i * 3 + 1], decoded[i * 3 + 2]],
                first,
                "pixel {i} differs from pixel 0 with 1x1 components"
            );
        }
    }
}
