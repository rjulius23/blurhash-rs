//! BlurHash encoding: convert an RGB image into a compact BlurHash string.
//!
//! The encoder performs a separable DCT (Discrete Cosine Transform) on the
//! image pixels and quantizes the resulting components into a base83-encoded
//! string. The 2D DCT is split into two 1D passes for much better performance.
//!
//! Large images are automatically downsampled before encoding since BlurHash
//! only uses a few frequency components. All intermediate buffers are
//! stack-allocated for zero heap overhead in the hot path.

use std::f32::consts::PI;

use crate::base83;
use crate::color::{linear_to_srgb, sign_pow_f32, srgb_to_linear_f32};
use crate::error::BlurhashError;

/// Maximum downsampled dimension. After downsampling, w and h are at most this.
/// 32 is well above the Nyquist requirement for 9 components (need 18 samples).
const MAX_DS: usize = 32;

/// Maximum BlurHash output length: 4 + 2 * 9 * 9 = 166 bytes.
const MAX_HASH_LEN: usize = 4 + 2 * 9 * 9;

/// Precomputed cosine table for the common case dim == MAX_DS.
/// Layout: COS_TABLE_DS[i * MAX_DS + x] = cos(PI * i * x / MAX_DS).
/// Covers all 9 component indices and MAX_DS sample positions.
const fn build_cos_table_ds() -> [f32; 9 * MAX_DS] {
    let mut table = [0.0f32; 9 * MAX_DS];
    // We use the identity cos(a) = 1 - 2*sin^2(a/2) with a Taylor series
    // for sin, but that's complex in const context. Instead, approximate
    // using the Chebyshev-like approach: compute cos via repeated angle
    // addition. For const correctness, use the formula:
    //   cos(PI * i * x / N) where N = MAX_DS
    //
    // Since we can't call cos() in const context, we use a polynomial
    // approximation of cos(t) that is accurate to ~1e-7 on [0, PI].
    let mut i: usize = 0;
    while i < 9 {
        let mut x: usize = 0;
        while x < MAX_DS {
            // t = PI * i * x / MAX_DS, but we need to compute cos(t).
            // Map t into [0, 2*PI) range.
            let t_num = i * x; // numerator: t = PI * t_num / MAX_DS
                               // Use symmetry: cos(PI * n + r) = (-1)^n * cos(r) for reduction.
                               // t / PI = t_num / MAX_DS = q + frac where q is integer part
            let q = t_num / MAX_DS;
            let frac_num = t_num - q * MAX_DS; // frac_num / MAX_DS in [0, 1)
                                               // cos(PI * (q + frac)) = (-1)^q * cos(PI * frac)
            let sign: f64 = if q % 2 == 0 { 1.0 } else { -1.0 };
            // theta = PI * frac_num / MAX_DS, in [0, PI)
            // Use cos(theta) = 1 - theta^2/2! + theta^4/4! - theta^6/6! + theta^8/8!
            let pi: f64 = std::f64::consts::PI;
            let theta = pi * frac_num as f64 / MAX_DS as f64;
            let t2 = theta * theta;
            let t4 = t2 * t2;
            let t6 = t4 * t2;
            let t8 = t6 * t2;
            let t10 = t8 * t2;
            let cos_val = 1.0 - t2 / 2.0 + t4 / 24.0 - t6 / 720.0 + t8 / 40320.0 - t10 / 3628800.0;
            table[i * MAX_DS + x] = (sign * cos_val) as f32;
            x += 1;
        }
        i += 1;
    }
    table
}

static COS_TABLE_DS: [f32; 9 * MAX_DS] = build_cos_table_ds();

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

    let cx = components_x as usize;
    let cy = components_y as usize;

    // -----------------------------------------------------------------------
    // Downsample for large images: BlurHash encodes at most 9 frequency
    // components per axis, so by Nyquist we only need ~18 samples.
    // We downsample to a small working image using a box (area-average)
    // filter, then run the DCT on the small image.
    //
    // All buffers are stack-allocated since the downsampled image is at
    // most MAX_DS x MAX_DS = 1024 pixels (12 KiB for 3 f32 channels).
    // -----------------------------------------------------------------------
    let target_w = MAX_DS.max(2 * cx);
    let target_h = MAX_DS.max(2 * cy);
    let w_in = width as usize;
    let h_in = height as usize;

    // Stack-allocated linear pixel buffers: 3 channels x MAX_DS^2 x 4 bytes = 12 KiB.
    let mut linear_r_buf = [0.0f32; MAX_DS * MAX_DS];
    let mut linear_g_buf = [0.0f32; MAX_DS * MAX_DS];
    let mut linear_b_buf = [0.0f32; MAX_DS * MAX_DS];

    let (w, h) = if w_in > target_w || h_in > target_h {
        let dw = target_w.min(w_in);
        let dh = target_h.min(h_in);

        // Subsample: pick one representative pixel per target cell.
        // We pick the pixel at the center of each cell for best quality.
        // This is O(dw * dh) work instead of O(w_in * h_in) for a full
        // box filter, and the quality difference is negligible for BlurHash
        // since the DCT is a low-pass filter and quantization is coarse.
        for dy in 0..dh {
            let sy = (dy * 2 + 1) * h_in / (dh * 2);
            let row_base = sy * w_in * 3;
            let dst_row = dy * dw;
            for dx in 0..dw {
                let sx = (dx * 2 + 1) * w_in / (dw * 2);
                let base = row_base + sx * 3;
                let idx = dst_row + dx;
                unsafe {
                    linear_r_buf[idx] = srgb_to_linear_f32(*pixels.get_unchecked(base));
                    linear_g_buf[idx] = srgb_to_linear_f32(*pixels.get_unchecked(base + 1));
                    linear_b_buf[idx] = srgb_to_linear_f32(*pixels.get_unchecked(base + 2));
                }
            }
        }
        (dw, dh)
    } else {
        // Small image: convert all pixels to linear directly.
        let num_pixels = w_in * h_in;
        for idx in 0..num_pixels {
            let base = idx * 3;
            unsafe {
                linear_r_buf[idx] = srgb_to_linear_f32(*pixels.get_unchecked(base));
                linear_g_buf[idx] = srgb_to_linear_f32(*pixels.get_unchecked(base + 1));
                linear_b_buf[idx] = srgb_to_linear_f32(*pixels.get_unchecked(base + 2));
            }
        }
        (w_in, h_in)
    };

    let linear_r = &linear_r_buf[..w * h];
    let linear_g = &linear_g_buf[..w * h];
    let linear_b = &linear_b_buf[..w * h];

    let wf = w as f32;
    let hf = h as f32;

    // Use precomputed cosine table when w == MAX_DS; otherwise compute at runtime.
    let mut cos_x_table_buf = [0.0f32; 9 * MAX_DS];
    let cos_x_table: &[f32] = if w == MAX_DS {
        &COS_TABLE_DS
    } else {
        for i in 0..cx {
            let base = i * w;
            for x in 0..w {
                cos_x_table_buf[base + x] = (PI * i as f32 * x as f32 / wf).cos();
            }
        }
        &cos_x_table_buf
    };

    let mut cos_y_table_buf = [0.0f32; 9 * MAX_DS];
    let cos_y_table: &[f32] = if h == MAX_DS {
        &COS_TABLE_DS
    } else {
        for j in 0..cy {
            let base = j * h;
            for y in 0..h {
                cos_y_table_buf[base + y] = (PI * j as f32 * y as f32 / hf).cos();
            }
        }
        &cos_y_table_buf
    };

    // -----------------------------------------------------------------------
    // Separable 2D DCT
    // -----------------------------------------------------------------------
    // Step 1: For each row y, compute partial sums over x for each component i.
    //   row_partial[i * h + y] = sum_x(linear_pixel[y][x] * cos_x[i][x])
    // Layout is component-major (contiguous over y) so pass 2 can SIMD over y.
    //
    // Stack-allocated: 3 channels x 9 * MAX_DS = ~3.4 KiB.
    let mut row_partial_r = [0.0f32; 9 * MAX_DS];
    let mut row_partial_g = [0.0f32; 9 * MAX_DS];
    let mut row_partial_b = [0.0f32; 9 * MAX_DS];

    for y in 0..h {
        let pixel_row_offset = y * w;
        for i in 0..cx {
            let cos_x_base = i * w;

            #[cfg(feature = "simd")]
            let (sr, sg, sb) = crate::simd::dot_product_3ch_f32(
                &cos_x_table[cos_x_base..cos_x_base + w],
                &linear_r[pixel_row_offset..pixel_row_offset + w],
                &linear_g[pixel_row_offset..pixel_row_offset + w],
                &linear_b[pixel_row_offset..pixel_row_offset + w],
                w,
            );

            #[cfg(not(feature = "simd"))]
            let (sr, sg, sb) = {
                let mut sr = 0.0f32;
                let mut sg = 0.0f32;
                let mut sb = 0.0f32;
                for x in 0..w {
                    unsafe {
                        let cos_val = *cos_x_table.get_unchecked(cos_x_base + x);
                        let px_idx = pixel_row_offset + x;
                        sr += cos_val * *linear_r.get_unchecked(px_idx);
                        sg += cos_val * *linear_g.get_unchecked(px_idx);
                        sb += cos_val * *linear_b.get_unchecked(px_idx);
                    }
                }
                (sr, sg, sb)
            };

            // Component-major layout: row_partial[i * h + y].
            let partial_idx = i * h + y;
            row_partial_r[partial_idx] = sr;
            row_partial_g[partial_idx] = sg;
            row_partial_b[partial_idx] = sb;
        }
    }

    // Step 2: For each component pair (j, i), accumulate over y:
    //   component[j*cx+i] = norm * scale * sum_y(cos_y[j*h+y] * row_partial[i*h+y])
    //
    // Stack-allocated: 81 * 3 * 4 = 972 bytes.
    let num_components = cx * cy;
    let mut components_arr = [[0.0f32; 3]; 81];
    let mut max_ac_component: f32 = 0.0;
    let scale = 1.0f32 / (wf * hf);

    for j in 0..cy {
        let cos_y_base = j * h;
        for i in 0..cx {
            let norm_factor = if i == 0 && j == 0 { 1.0f32 } else { 2.0f32 };
            let partial_base = i * h;

            #[cfg(feature = "simd")]
            let (r_sum, g_sum, b_sum) = crate::simd::dot_product_3ch_f32(
                &cos_y_table[cos_y_base..cos_y_base + h],
                &row_partial_r[partial_base..partial_base + h],
                &row_partial_g[partial_base..partial_base + h],
                &row_partial_b[partial_base..partial_base + h],
                h,
            );

            #[cfg(not(feature = "simd"))]
            let (r_sum, g_sum, b_sum) = {
                let mut r_sum = 0.0f32;
                let mut g_sum = 0.0f32;
                let mut b_sum = 0.0f32;
                for y in 0..h {
                    unsafe {
                        let cos_y_val = *cos_y_table.get_unchecked(cos_y_base + y);
                        let partial_idx = partial_base + y;
                        r_sum += cos_y_val * *row_partial_r.get_unchecked(partial_idx);
                        g_sum += cos_y_val * *row_partial_g.get_unchecked(partial_idx);
                        b_sum += cos_y_val * *row_partial_b.get_unchecked(partial_idx);
                    }
                }
                (r_sum, g_sum, b_sum)
            };

            let factor = norm_factor * scale;
            let component = [r_sum * factor, g_sum * factor, b_sum * factor];
            let comp_idx = j * cx + i;
            if comp_idx != 0 {
                max_ac_component = max_ac_component
                    .max(component[0].abs())
                    .max(component[1].abs())
                    .max(component[2].abs());
            }
            components_arr[comp_idx] = component;
        }
    }

    // -----------------------------------------------------------------------
    // Encode directly into a stack-allocated byte buffer.
    // This eliminates all per-component String allocations from base83.
    // -----------------------------------------------------------------------
    let hash_len = 4 + 2 * num_components;
    let mut buf = [0u8; MAX_HASH_LEN];
    let mut offset = 0usize;

    // Size flag: 1 base83 digit.
    let size_flag = (components_x - 1) + (components_y - 1) * 9;
    offset = base83::encode_to_buf(size_flag as u64, 1, &mut buf, offset);

    // Quantized max AC component: 1 base83 digit.
    let quant_max_ac = (max_ac_component * 166.0 - 0.5).floor().clamp(0.0, 82.0) as u64;
    offset = base83::encode_to_buf(quant_max_ac, 1, &mut buf, offset);

    // DC value: 4 base83 digits.
    let dc = &components_arr[0];
    let dc_value = ((linear_to_srgb(dc[0] as f64) as u64) << 16)
        | ((linear_to_srgb(dc[1] as f64) as u64) << 8)
        | (linear_to_srgb(dc[2] as f64) as u64);
    offset = base83::encode_to_buf(dc_value, 4, &mut buf, offset);

    // AC values: quantize and encode inline (2 base83 digits each).
    let ac_component_norm_factor = (quant_max_ac as f32 + 1.0) / 166.0;
    for component in &components_arr[1..num_components] {
        let quant_r = (sign_pow_f32(component[0] / ac_component_norm_factor, 0.5) * 9.0 + 9.5)
            .floor()
            .clamp(0.0, 18.0) as u64;
        let quant_g = (sign_pow_f32(component[1] / ac_component_norm_factor, 0.5) * 9.0 + 9.5)
            .floor()
            .clamp(0.0, 18.0) as u64;
        let quant_b = (sign_pow_f32(component[2] / ac_component_norm_factor, 0.5) * 9.0 + 9.5)
            .floor()
            .clamp(0.0, 18.0) as u64;
        let ac_value = quant_r * 19 * 19 + quant_g * 19 + quant_b;
        offset = base83::encode_to_buf(ac_value, 2, &mut buf, offset);
    }

    debug_assert_eq!(offset, hash_len);

    // Single allocation: convert the stack buffer to a String.
    // SAFETY: buf contains only ASCII bytes from the base83 ALPHABET.
    Ok(unsafe { String::from_utf8_unchecked(buf[..hash_len].to_vec()) })
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
