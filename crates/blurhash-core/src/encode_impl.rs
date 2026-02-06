//! BlurHash encoding: convert an RGB image into a compact BlurHash string.
//!
//! The encoder performs a DCT (Discrete Cosine Transform) on the image pixels
//! and quantizes the resulting components into a base83-encoded string.

use std::f64::consts::PI;

use crate::base83;
use crate::color::{linear_to_srgb, sign_pow, srgb_to_linear};
use crate::error::BlurhashError;

/// Encode an RGB image into a BlurHash string.
///
/// # Arguments
///
/// * `pixels` - Flat RGB byte array in row-major order (3 bytes per pixel).
/// * `width` - Image width in pixels.
/// * `height` - Image height in pixels.
/// * `components_x` - Number of horizontal components (1..=9).
/// * `components_y` - Number of vertical components (1..=9).
///
/// # Errors
///
/// Returns an error if the component counts are out of range or if the pixel
/// buffer length does not match width * height * 3.
///
/// # Examples
///
/// ```
/// use blurhash_core::encode;
/// // A 2x2 red image
/// let pixels = [255, 0, 0, 255, 0, 0, 255, 0, 0, 255, 0, 0];
/// let hash = encode(&pixels, 2, 2, 4, 3).unwrap();
/// assert!(!hash.is_empty());
/// ```
pub fn encode(
    pixels: &[u8],
    width: u32,
    height: u32,
    components_x: u32,
    components_y: u32,
) -> Result<String, BlurhashError> {
    if width == 0 || height == 0 {
        return Err(BlurhashError::InvalidDimensions {
            width,
            height,
            reason: "width and height must be > 0",
        });
    }

    // Cap dimensions to prevent DoS via excessive CPU and memory usage.
    const MAX_DIMENSION: u32 = 10_000;
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(BlurhashError::InvalidDimensions {
            width,
            height,
            reason: "dimensions must be <= 10000",
        });
    }

    if !(1..=9).contains(&components_x) {
        return Err(BlurhashError::InvalidComponentCount {
            component: "x",
            value: components_x,
        });
    }
    if !(1..=9).contains(&components_y) {
        return Err(BlurhashError::InvalidComponentCount {
            component: "y",
            value: components_y,
        });
    }

    let expected_len = (width as u64)
        .checked_mul(height as u64)
        .and_then(|v| v.checked_mul(3))
        .and_then(|v| usize::try_from(v).ok())
        .ok_or(BlurhashError::InvalidDimensions {
            width,
            height,
            reason: "dimensions overflow buffer size calculation",
        })?;
    if pixels.len() != expected_len {
        return Err(BlurhashError::EncodingError(format!(
            "pixel buffer length {} does not match {}x{}x3 = {}",
            pixels.len(),
            width,
            height,
            expected_len
        )));
    }

    let w = width as usize;
    let h = height as usize;
    let wf = width as f64;
    let hf = height as f64;

    // Precompute cosine tables.
    // cos_x[i][x] = cos(PI * i * x / width)
    let cos_x: Vec<Vec<f64>> = (0..components_x as usize)
        .map(|i| {
            (0..w)
                .map(|x| (PI * i as f64 * x as f64 / wf).cos())
                .collect()
        })
        .collect();
    // cos_y[j][y] = cos(PI * j * y / height)
    let cos_y: Vec<Vec<f64>> = (0..components_y as usize)
        .map(|j| {
            (0..h)
                .map(|y| (PI * j as f64 * y as f64 / hf).cos())
                .collect()
        })
        .collect();

    // Convert image to linear RGB (using LUT).
    let linear_pixels: Vec<[f64; 3]> = (0..w * h)
        .map(|idx| {
            let base = idx * 3;
            [
                srgb_to_linear(pixels[base]),
                srgb_to_linear(pixels[base + 1]),
                srgb_to_linear(pixels[base + 2]),
            ]
        })
        .collect();

    // Compute DCT components.
    let num_components = (components_x * components_y) as usize;
    let mut components: Vec<[f64; 3]> = Vec::with_capacity(num_components);
    let mut max_ac_component: f64 = 0.0;
    let scale = 1.0 / (wf * hf);

    for (j, cos_y_row) in cos_y.iter().enumerate() {
        for (i, cos_x_row) in cos_x.iter().enumerate() {
            let norm_factor = if i == 0 && j == 0 { 1.0 } else { 2.0 };
            let mut r_sum = 0.0f64;
            let mut g_sum = 0.0f64;
            let mut b_sum = 0.0f64;

            for (y, &cos_y_val) in cos_y_row.iter().enumerate() {
                let row_offset = y * w;
                for (x, &cos_x_val) in cos_x_row.iter().enumerate() {
                    let basis = norm_factor * cos_x_val * cos_y_val;
                    let px = &linear_pixels[row_offset + x];
                    r_sum += basis * px[0];
                    g_sum += basis * px[1];
                    b_sum += basis * px[2];
                }
            }

            let component = [r_sum * scale, g_sum * scale, b_sum * scale];
            if i != 0 || j != 0 {
                max_ac_component = max_ac_component
                    .max(component[0].abs())
                    .max(component[1].abs())
                    .max(component[2].abs());
            }
            components.push(component);
        }
    }

    // Encode the DC value.
    let dc = &components[0];
    let dc_value = ((linear_to_srgb(dc[0]) as u64) << 16)
        | ((linear_to_srgb(dc[1]) as u64) << 8)
        | (linear_to_srgb(dc[2]) as u64);

    // Quantize the maximum AC component.
    let quant_max_ac = (max_ac_component * 166.0 - 0.5)
        .floor()
        .clamp(0.0, 82.0) as u64;
    let ac_component_norm_factor = (quant_max_ac as f64 + 1.0) / 166.0;

    // Encode AC values.
    let mut ac_values: Vec<u64> = Vec::with_capacity(num_components - 1);
    for component in &components[1..] {
        let quant_r = (sign_pow(component[0] / ac_component_norm_factor, 0.5) * 9.0 + 9.5)
            .floor()
            .clamp(0.0, 18.0) as u64;
        let quant_g = (sign_pow(component[1] / ac_component_norm_factor, 0.5) * 9.0 + 9.5)
            .floor()
            .clamp(0.0, 18.0) as u64;
        let quant_b = (sign_pow(component[2] / ac_component_norm_factor, 0.5) * 9.0 + 9.5)
            .floor()
            .clamp(0.0, 18.0) as u64;
        ac_values.push(quant_r * 19 * 19 + quant_g * 19 + quant_b);
    }

    // Build the BlurHash string.
    let size_flag = (components_x - 1) + (components_y - 1) * 9;
    let estimated_len = 4 + 2 * num_components;
    let mut result = String::with_capacity(estimated_len);

    result.push_str(&base83::encode(size_flag as u64, 1)?);
    result.push_str(&base83::encode(quant_max_ac, 1)?);
    result.push_str(&base83::encode(dc_value, 4)?);
    for ac_value in &ac_values {
        result.push_str(&base83::encode(*ac_value, 2)?);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_solid_black() {
        // A 4x4 solid black image
        let pixels = vec![0u8; 4 * 4 * 3];
        let hash = encode(&pixels, 4, 4, 4, 3).unwrap();
        assert!(!hash.is_empty());
        // Size flag for 4x3: (4-1) + (3-1)*9 = 3 + 18 = 21
        // First character encodes 21
        let size_info = base83::decode(&hash[0..1]).unwrap();
        assert_eq!(size_info, 21);
    }

    #[test]
    fn test_encode_solid_white() {
        // A 4x4 solid white image
        let pixels = vec![255u8; 4 * 4 * 3];
        let hash = encode(&pixels, 4, 4, 4, 3).unwrap();
        assert!(!hash.is_empty());
    }

    #[test]
    fn test_encode_solid_red() {
        // A 2x2 solid red image
        let mut pixels = vec![0u8; 2 * 2 * 3];
        for i in 0..4 {
            pixels[i * 3] = 255; // R
        }
        let hash = encode(&pixels, 2, 2, 4, 3).unwrap();
        assert!(!hash.is_empty());
        // Length should be 4 + 2*4*3 = 28
        assert_eq!(hash.len(), 4 + 2 * 4 * 3);
    }

    #[test]
    fn test_encode_component_count_validation() {
        let pixels = vec![0u8; 4 * 4 * 3];
        assert!(encode(&pixels, 4, 4, 0, 3).is_err());
        assert!(encode(&pixels, 4, 4, 10, 3).is_err());
        assert!(encode(&pixels, 4, 4, 4, 0).is_err());
        assert!(encode(&pixels, 4, 4, 4, 10).is_err());
    }

    #[test]
    fn test_encode_pixel_buffer_validation() {
        let pixels = vec![0u8; 10]; // wrong length
        assert!(encode(&pixels, 4, 4, 4, 3).is_err());
    }

    #[test]
    fn test_encode_hash_length() {
        // For components_x=4, components_y=3: length = 4 + 2*4*3 = 28
        let pixels = vec![128u8; 4 * 4 * 3];
        let hash = encode(&pixels, 4, 4, 4, 3).unwrap();
        assert_eq!(hash.len(), 28);
    }

    #[test]
    fn test_encode_1x1_components() {
        let pixels = vec![100u8; 2 * 2 * 3];
        let hash = encode(&pixels, 2, 2, 1, 1).unwrap();
        // Length = 4 + 2*1*1 = 6
        assert_eq!(hash.len(), 6);
    }

    #[test]
    fn test_encode_gradient() {
        // Horizontal gradient
        let mut pixels = vec![0u8; 8 * 3];
        for x in 0..8 {
            let val = (x * 32).min(255) as u8;
            pixels[x * 3] = val;
            pixels[x * 3 + 1] = val;
            pixels[x * 3 + 2] = val;
        }
        let hash = encode(&pixels, 8, 1, 4, 1).unwrap();
        assert!(!hash.is_empty());
    }
}
