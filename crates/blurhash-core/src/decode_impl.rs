//! BlurHash decoding: convert a BlurHash string back into an RGB image.
//!
//! The decoder parses the base83-encoded BlurHash string, extracts the DCT
//! components, and reconstructs an image of the specified dimensions using a
//! separable inverse DCT (two 1D passes) for much better performance.
//!
//! When the `parallel` feature is enabled, large images are decoded using
//! rayon's work-stealing thread pool for improved throughput.

use std::f32::consts::PI;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::base83;
use crate::color::{linear_to_srgb_f32, sign_pow_f32, srgb_to_linear_f32};
use crate::error::BlurhashError;

/// Minimum number of output pixels (width * height) before we use parallel decoding.
#[cfg(feature = "parallel")]
const PARALLEL_PIXEL_THRESHOLD: usize = 4096; // ~64x64

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
    if width == 0 || height == 0 {
        return Err(BlurhashError::InvalidDimensions {
            width,
            height,
            reason: "width and height must be > 0",
        });
    }

    // Cap dimensions to prevent DoS via excessive memory allocation.
    // 10000x10000x3 = 300 MB which is a reasonable upper bound.
    const MAX_DIMENSION: u32 = 10_000;
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(BlurhashError::InvalidDimensions {
            width,
            height,
            reason: "dimensions must be <= 10000",
        });
    }

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
    let real_max_value = (quant_max_value as f32 + 1.0) / 166.0 * punch as f32;

    // Decode DC component.
    let dc_value = base83::decode(&blurhash[2..6])?;
    let dc_r = srgb_to_linear_f32(((dc_value >> 16) & 255) as u8);
    let dc_g = srgb_to_linear_f32(((dc_value >> 8) & 255) as u8);
    let dc_b = srgb_to_linear_f32((dc_value & 255) as u8);

    let sx = size_x as usize;
    let sy = size_y as usize;
    let num_components = sx * sy;

    // Store colours as flat [f32; 3] arrays.
    // colours[idx] = [r, g, b] for component idx = i + j * size_x.
    let mut colours = vec![[0.0f32; 3]; num_components];
    colours[0] = [dc_r, dc_g, dc_b];

    // Decode AC components using fast f32 sign_pow (x*x path for exp=2.0).
    for component_idx in 1..num_components {
        let start = 4 + component_idx * 2;
        let ac_value = base83::decode(&blurhash[start..start + 2])?;

        let quant_r = (ac_value / (19 * 19)) as f32;
        let quant_g = ((ac_value / 19) % 19) as f32;
        let quant_b = (ac_value % 19) as f32;

        colours[component_idx] = [
            sign_pow_f32((quant_r - 9.0) / 9.0, 2.0) * real_max_value,
            sign_pow_f32((quant_g - 9.0) / 9.0, 2.0) * real_max_value,
            sign_pow_f32((quant_b - 9.0) / 9.0, 2.0) * real_max_value,
        ];
    }

    let w = width as usize;
    let h = height as usize;
    let wf = width as f32;
    let hf = height as f32;

    // Precompute cosine tables as flat arrays for cache-friendly access.
    // cos_x_table[i * w + x] = cos(PI * x * i / width)
    let mut cos_x_table = vec![0.0f32; sx * w];
    for i in 0..sx {
        let base = i * w;
        for x in 0..w {
            // SAFETY: base + x = i * w + x < sx * w, always in bounds.
            unsafe {
                *cos_x_table.get_unchecked_mut(base + x) =
                    (PI * x as f32 * i as f32 / wf).cos();
            }
        }
    }

    // cos_y_table[j * h + y] = cos(PI * y * j / height)
    let mut cos_y_table = vec![0.0f32; sy * h];
    for j in 0..sy {
        let base = j * h;
        for y in 0..h {
            // SAFETY: base + y = j * h + y < sy * h, always in bounds.
            unsafe {
                *cos_y_table.get_unchecked_mut(base + y) =
                    (PI * y as f32 * j as f32 / hf).cos();
            }
        }
    }

    // -----------------------------------------------------------------------
    // Separable inverse DCT
    // -----------------------------------------------------------------------
    // Step 1: For each component row j, compute partial sums over x for each pixel column x.
    //   partial[j * w + x] = sum_i(colours[j * sx + i] * cos_x[i * w + x])
    // This gives us partial_r, partial_g, partial_b, each [sy][w].
    let mut partial_r = vec![0.0f32; sy * w];
    let mut partial_g = vec![0.0f32; sy * w];
    let mut partial_b = vec![0.0f32; sy * w];

    for j in 0..sy {
        let colour_row_base = j * sx;
        let partial_row_base = j * w;
        for x in 0..w {
            let mut sr = 0.0f32;
            let mut sg = 0.0f32;
            let mut sb = 0.0f32;
            for i in 0..sx {
                // SAFETY: i * w + x < sx * w; colour_row_base + i < sy * sx = num_components.
                unsafe {
                    let cos_val = *cos_x_table.get_unchecked(i * w + x);
                    let colour = *colours.get_unchecked(colour_row_base + i);
                    sr += colour[0] * cos_val;
                    sg += colour[1] * cos_val;
                    sb += colour[2] * cos_val;
                }
            }
            // SAFETY: partial_row_base + x = j * w + x < sy * w.
            unsafe {
                *partial_r.get_unchecked_mut(partial_row_base + x) = sr;
                *partial_g.get_unchecked_mut(partial_row_base + x) = sg;
                *partial_b.get_unchecked_mut(partial_row_base + x) = sb;
            }
        }
    }

    // Step 2: For each pixel (x, y), accumulate over j:
    //   pixel[y][x] = sum_j(partial[j][x] * cos_y[j * h + y])
    // Then convert linear -> sRGB.
    let mut result = vec![0u8; w * h * 3];

    // Pre-gather cos_y values per row for SIMD-friendly access.
    // cos_y_per_row[y * sy + j] = cos_y_table[j * h + y]
    #[cfg(feature = "simd")]
    let cos_y_per_row: Vec<f32> = {
        let mut table = vec![0.0f32; h * sy];
        for y in 0..h {
            for j in 0..sy {
                table[y * sy + j] = cos_y_table[j * h + y];
            }
        }
        table
    };

    let decode_row = |y: usize, row: &mut [u8]| {
        #[cfg(feature = "simd")]
        {
            let cos_y_vals = &cos_y_per_row[y * sy..(y + 1) * sy];
            crate::simd::decode_accumulate_row(
                cos_y_vals,
                &partial_r,
                &partial_g,
                &partial_b,
                w,
                sy,
                row,
                linear_to_srgb_f32,
            );
        }

        #[cfg(not(feature = "simd"))]
        {
            for x in 0..w {
                let mut pr = 0.0f32;
                let mut pg = 0.0f32;
                let mut pb = 0.0f32;
                for j in 0..sy {
                    // SAFETY: j * h + y < sy * h; j * w + x < sy * w.
                    unsafe {
                        let cos_y_val = *cos_y_table.get_unchecked(j * h + y);
                        let partial_idx = j * w + x;
                        pr += cos_y_val * *partial_r.get_unchecked(partial_idx);
                        pg += cos_y_val * *partial_g.get_unchecked(partial_idx);
                        pb += cos_y_val * *partial_b.get_unchecked(partial_idx);
                    }
                }
                let idx = x * 3;
                // SAFETY: idx + 2 = x * 3 + 2 < w * 3 = row.len().
                unsafe {
                    *row.get_unchecked_mut(idx) = linear_to_srgb_f32(pr);
                    *row.get_unchecked_mut(idx + 1) = linear_to_srgb_f32(pg);
                    *row.get_unchecked_mut(idx + 2) = linear_to_srgb_f32(pb);
                }
            }
        }
    };

    let row_bytes = w * 3;

    #[cfg(feature = "parallel")]
    {
        if w * h >= PARALLEL_PIXEL_THRESHOLD {
            result
                .par_chunks_mut(row_bytes)
                .enumerate()
                .for_each(|(y, row)| decode_row(y, row));
        } else {
            for (y, row) in result.chunks_mut(row_bytes).enumerate() {
                decode_row(y, row);
            }
        }
    }

    #[cfg(not(feature = "parallel"))]
    {
        for (y, row) in result.chunks_mut(row_bytes).enumerate() {
            decode_row(y, row);
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
        // Verify decode produced non-trivial output (not all zeros)
        assert!(pixels.iter().any(|&p| p > 0));
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
