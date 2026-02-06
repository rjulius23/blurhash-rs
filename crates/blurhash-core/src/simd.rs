//! SIMD-accelerated inner loops for BlurHash encode/decode.
//!
//! Provides platform-specific SIMD implementations for the hot
//! multiply-accumulate loops in the separable DCT. Falls back to
//! scalar code on unsupported platforms.


// ---------------------------------------------------------------------------
// Encode pass 1: dot product of cos_x row with pixel channel row
// ---------------------------------------------------------------------------

/// Compute the dot product of `cos_row[0..len]` and `pixel_row[0..len]`.
///
/// This is the encode pass-1 inner loop:
///   partial = sum_x(cos_x[i*w + x] * linear_channel[y*w + x])
///
/// # Safety
///
/// `cos_row` and `pixel_row` must both have at least `len` elements.
#[inline]
pub fn dot_product_f32(cos_row: &[f32], pixel_row: &[f32], len: usize) -> f32 {
    debug_assert!(cos_row.len() >= len);
    debug_assert!(pixel_row.len() >= len);

    #[cfg(target_arch = "aarch64")]
    {
        // NEON is always available on aarch64.
        unsafe { dot_product_neon(cos_row, pixel_row, len) }
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe { dot_product_avx2(cos_row, pixel_row, len) }
        } else {
            dot_product_scalar(cos_row, pixel_row, len)
        }
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        dot_product_scalar(cos_row, pixel_row, len)
    }
}

/// Scalar fallback for dot product.
#[inline]
fn dot_product_scalar(cos_row: &[f32], pixel_row: &[f32], len: usize) -> f32 {
    let mut sum = 0.0f32;
    for i in 0..len {
        unsafe {
            sum += *cos_row.get_unchecked(i) * *pixel_row.get_unchecked(i);
        }
    }
    sum
}

/// NEON-accelerated dot product using float32x4 FMA.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn dot_product_neon(cos_row: &[f32], pixel_row: &[f32], len: usize) -> f32 {
    use std::arch::aarch64::*;

    let cos_ptr = cos_row.as_ptr();
    let pix_ptr = pixel_row.as_ptr();

    let mut acc0 = vdupq_n_f32(0.0);
    let mut acc1 = vdupq_n_f32(0.0);

    let chunks = len / 8;
    let remainder = len % 8;

    // Process 8 elements at a time (2 x float32x4).
    for c in 0..chunks {
        let offset = c * 8;
        let c0 = vld1q_f32(cos_ptr.add(offset));
        let p0 = vld1q_f32(pix_ptr.add(offset));
        acc0 = vfmaq_f32(acc0, c0, p0);

        let c1 = vld1q_f32(cos_ptr.add(offset + 4));
        let p1 = vld1q_f32(pix_ptr.add(offset + 4));
        acc1 = vfmaq_f32(acc1, c1, p1);
    }

    // Process remaining 4-element chunk.
    let mut tail_start = chunks * 8;
    if remainder >= 4 {
        let c0 = vld1q_f32(cos_ptr.add(tail_start));
        let p0 = vld1q_f32(pix_ptr.add(tail_start));
        acc0 = vfmaq_f32(acc0, c0, p0);
        tail_start += 4;
    }

    // Combine the two accumulators.
    acc0 = vaddq_f32(acc0, acc1);

    // Horizontal sum of the 4 lanes.
    let mut sum = vaddvq_f32(acc0);

    // Handle remaining scalar elements.
    for i in tail_start..len {
        sum += *cos_ptr.add(i) * *pix_ptr.add(i);
    }

    sum
}

/// AVX2-accelerated dot product using float32x8 FMA.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2", enable = "fma")]
unsafe fn dot_product_avx2(cos_row: &[f32], pixel_row: &[f32], len: usize) -> f32 {
    use std::arch::x86_64::*;

    let cos_ptr = cos_row.as_ptr();
    let pix_ptr = pixel_row.as_ptr();

    let mut acc0 = _mm256_setzero_ps();
    let mut acc1 = _mm256_setzero_ps();

    let chunks = len / 16;
    let remainder = len % 16;

    // Process 16 elements at a time (2 x f32x8).
    for c in 0..chunks {
        let offset = c * 16;
        let c0 = _mm256_loadu_ps(cos_ptr.add(offset));
        let p0 = _mm256_loadu_ps(pix_ptr.add(offset));
        acc0 = _mm256_fmadd_ps(c0, p0, acc0);

        let c1 = _mm256_loadu_ps(cos_ptr.add(offset + 8));
        let p1 = _mm256_loadu_ps(pix_ptr.add(offset + 8));
        acc1 = _mm256_fmadd_ps(c1, p1, acc1);
    }

    // Process remaining 8-element chunk.
    let mut tail_start = chunks * 16;
    if remainder >= 8 {
        let c0 = _mm256_loadu_ps(cos_ptr.add(tail_start));
        let p0 = _mm256_loadu_ps(pix_ptr.add(tail_start));
        acc0 = _mm256_fmadd_ps(c0, p0, acc0);
        tail_start += 8;
    }

    // Combine accumulators.
    acc0 = _mm256_add_ps(acc0, acc1);

    // Horizontal sum of 8 lanes.
    // Add high 128 to low 128.
    let hi = _mm256_extractf128_ps(acc0, 1);
    let lo = _mm256_castps256_ps128(acc0);
    let sum128 = _mm_add_ps(lo, hi);
    // Horizontal add within 128: [a+b, c+d, a+b, c+d]
    let shuf = _mm_movehdup_ps(sum128);
    let sums = _mm_add_ps(sum128, shuf);
    let shuf2 = _mm_movehl_ps(sums, sums);
    let sums2 = _mm_add_ss(sums, shuf2);
    let mut sum = _mm_cvtss_f32(sums2);

    // Handle remaining scalar elements.
    for i in tail_start..len {
        sum += *cos_ptr.add(i) * *pix_ptr.add(i);
    }

    sum
}

// ---------------------------------------------------------------------------
// Encode pass 1: three-channel dot product (R, G, B simultaneously)
// ---------------------------------------------------------------------------

/// Compute three dot products simultaneously (one per color channel).
///
/// Returns (sum_r, sum_g, sum_b) where:
///   sum_c = sum_x(cos_row[x] * channel[x]) for c in {r, g, b}
///
/// # Safety
///
/// All slices must have at least `len` elements.
#[inline]
pub fn dot_product_3ch_f32(
    cos_row: &[f32],
    r_row: &[f32],
    g_row: &[f32],
    b_row: &[f32],
    len: usize,
) -> (f32, f32, f32) {
    debug_assert!(cos_row.len() >= len);
    debug_assert!(r_row.len() >= len);
    debug_assert!(g_row.len() >= len);
    debug_assert!(b_row.len() >= len);

    #[cfg(target_arch = "aarch64")]
    {
        unsafe { dot_product_3ch_neon(cos_row, r_row, g_row, b_row, len) }
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe { dot_product_3ch_avx2(cos_row, r_row, g_row, b_row, len) }
        } else {
            dot_product_3ch_scalar(cos_row, r_row, g_row, b_row, len)
        }
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        dot_product_3ch_scalar(cos_row, r_row, g_row, b_row, len)
    }
}

/// Scalar fallback for 3-channel dot product.
#[inline]
fn dot_product_3ch_scalar(
    cos_row: &[f32],
    r_row: &[f32],
    g_row: &[f32],
    b_row: &[f32],
    len: usize,
) -> (f32, f32, f32) {
    let mut sr = 0.0f32;
    let mut sg = 0.0f32;
    let mut sb = 0.0f32;
    for i in 0..len {
        unsafe {
            let c = *cos_row.get_unchecked(i);
            sr += c * *r_row.get_unchecked(i);
            sg += c * *g_row.get_unchecked(i);
            sb += c * *b_row.get_unchecked(i);
        }
    }
    (sr, sg, sb)
}

/// NEON-accelerated 3-channel dot product.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn dot_product_3ch_neon(
    cos_row: &[f32],
    r_row: &[f32],
    g_row: &[f32],
    b_row: &[f32],
    len: usize,
) -> (f32, f32, f32) {
    use std::arch::aarch64::*;

    let cp = cos_row.as_ptr();
    let rp = r_row.as_ptr();
    let gp = g_row.as_ptr();
    let bp = b_row.as_ptr();

    let mut acc_r = vdupq_n_f32(0.0);
    let mut acc_g = vdupq_n_f32(0.0);
    let mut acc_b = vdupq_n_f32(0.0);

    let chunks = len / 4;
    let tail_start = chunks * 4;

    for c in 0..chunks {
        let offset = c * 4;
        let cv = vld1q_f32(cp.add(offset));
        let rv = vld1q_f32(rp.add(offset));
        let gv = vld1q_f32(gp.add(offset));
        let bv = vld1q_f32(bp.add(offset));
        acc_r = vfmaq_f32(acc_r, cv, rv);
        acc_g = vfmaq_f32(acc_g, cv, gv);
        acc_b = vfmaq_f32(acc_b, cv, bv);
    }

    let mut sr = vaddvq_f32(acc_r);
    let mut sg = vaddvq_f32(acc_g);
    let mut sb = vaddvq_f32(acc_b);

    for i in tail_start..len {
        let c = *cp.add(i);
        sr += c * *rp.add(i);
        sg += c * *gp.add(i);
        sb += c * *bp.add(i);
    }

    (sr, sg, sb)
}

/// AVX2-accelerated 3-channel dot product.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2", enable = "fma")]
unsafe fn dot_product_3ch_avx2(
    cos_row: &[f32],
    r_row: &[f32],
    g_row: &[f32],
    b_row: &[f32],
    len: usize,
) -> (f32, f32, f32) {
    use std::arch::x86_64::*;

    let cp = cos_row.as_ptr();
    let rp = r_row.as_ptr();
    let gp = g_row.as_ptr();
    let bp = b_row.as_ptr();

    let mut acc_r = _mm256_setzero_ps();
    let mut acc_g = _mm256_setzero_ps();
    let mut acc_b = _mm256_setzero_ps();

    let chunks = len / 8;
    let tail_start = chunks * 8;

    for c in 0..chunks {
        let offset = c * 8;
        let cv = _mm256_loadu_ps(cp.add(offset));
        let rv = _mm256_loadu_ps(rp.add(offset));
        let gv = _mm256_loadu_ps(gp.add(offset));
        let bv = _mm256_loadu_ps(bp.add(offset));
        acc_r = _mm256_fmadd_ps(cv, rv, acc_r);
        acc_g = _mm256_fmadd_ps(cv, gv, acc_g);
        acc_b = _mm256_fmadd_ps(cv, bv, acc_b);
    }

    // Horizontal sum helper for AVX2.
    #[inline(always)]
    unsafe fn hsum_avx2(v: __m256) -> f32 {
        let hi = _mm256_extractf128_ps(v, 1);
        let lo = _mm256_castps256_ps128(v);
        let sum128 = _mm_add_ps(lo, hi);
        let shuf = _mm_movehdup_ps(sum128);
        let sums = _mm_add_ps(sum128, shuf);
        let shuf2 = _mm_movehl_ps(sums, sums);
        let sums2 = _mm_add_ss(sums, shuf2);
        _mm_cvtss_f32(sums2)
    }

    let mut sr = hsum_avx2(acc_r);
    let mut sg = hsum_avx2(acc_g);
    let mut sb = hsum_avx2(acc_b);

    for i in tail_start..len {
        let c = *cp.add(i);
        sr += c * *rp.add(i);
        sg += c * *gp.add(i);
        sb += c * *bp.add(i);
    }

    (sr, sg, sb)
}

// ---------------------------------------------------------------------------
// Decode pass 2: weighted sum of partial rows for a single output row
// ---------------------------------------------------------------------------

/// For each x in 0..width, compute:
///   out_r[x] = sum_j(cos_y_vals[j] * partial_r[j * width + x])
///   out_g[x] = sum_j(cos_y_vals[j] * partial_g[j * width + x])
///   out_b[x] = sum_j(cos_y_vals[j] * partial_b[j * width + x])
///
/// `cos_y_vals` has `num_j` elements, one per component row.
/// `partial_r/g/b` are flat arrays of shape [num_j][width].
/// Output is written into `out_rgb` as interleaved [R, G, B, R, G, B, ...].
///
/// # Safety
///
/// All slices must be correctly sized.
#[inline]
pub fn decode_accumulate_row(
    cos_y_vals: &[f32],
    partial_r: &[f32],
    partial_g: &[f32],
    partial_b: &[f32],
    width: usize,
    num_j: usize,
    out_rgb: &mut [u8],
    linear_to_srgb_fn: fn(f32) -> u8,
) {
    debug_assert!(cos_y_vals.len() >= num_j);
    debug_assert!(out_rgb.len() >= width * 3);

    #[cfg(target_arch = "aarch64")]
    {
        unsafe {
            decode_accumulate_row_neon(
                cos_y_vals,
                partial_r,
                partial_g,
                partial_b,
                width,
                num_j,
                out_rgb,
                linear_to_srgb_fn,
            );
        }
        return;
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        decode_accumulate_row_scalar(
            cos_y_vals,
            partial_r,
            partial_g,
            partial_b,
            width,
            num_j,
            out_rgb,
            linear_to_srgb_fn,
        );
    }
}

/// Scalar fallback for decode row accumulation.
#[inline]
fn decode_accumulate_row_scalar(
    cos_y_vals: &[f32],
    partial_r: &[f32],
    partial_g: &[f32],
    partial_b: &[f32],
    width: usize,
    num_j: usize,
    out_rgb: &mut [u8],
    linear_to_srgb_fn: fn(f32) -> u8,
) {
    for x in 0..width {
        let mut pr = 0.0f32;
        let mut pg = 0.0f32;
        let mut pb = 0.0f32;
        for j in 0..num_j {
            unsafe {
                let cy = *cos_y_vals.get_unchecked(j);
                let idx = j * width + x;
                pr += cy * *partial_r.get_unchecked(idx);
                pg += cy * *partial_g.get_unchecked(idx);
                pb += cy * *partial_b.get_unchecked(idx);
            }
        }
        let out_idx = x * 3;
        unsafe {
            *out_rgb.get_unchecked_mut(out_idx) = linear_to_srgb_fn(pr);
            *out_rgb.get_unchecked_mut(out_idx + 1) = linear_to_srgb_fn(pg);
            *out_rgb.get_unchecked_mut(out_idx + 2) = linear_to_srgb_fn(pb);
        }
    }
}

/// NEON-accelerated decode row accumulation.
///
/// For each x, sum over j: cos_y[j] * partial[j*w+x].
/// We vectorize over x (processing 4 pixels at a time), with the
/// j loop as the inner scalar broadcast.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn decode_accumulate_row_neon(
    cos_y_vals: &[f32],
    partial_r: &[f32],
    partial_g: &[f32],
    partial_b: &[f32],
    width: usize,
    num_j: usize,
    out_rgb: &mut [u8],
    linear_to_srgb_fn: fn(f32) -> u8,
) {
    use std::arch::aarch64::*;

    let chunks = width / 4;
    let tail_start = chunks * 4;

    for chunk in 0..chunks {
        let x = chunk * 4;
        let mut acc_r = vdupq_n_f32(0.0);
        let mut acc_g = vdupq_n_f32(0.0);
        let mut acc_b = vdupq_n_f32(0.0);

        for j in 0..num_j {
            let cy = vdupq_n_f32(*cos_y_vals.get_unchecked(j));
            let base = j * width + x;
            let rv = vld1q_f32(partial_r.as_ptr().add(base));
            let gv = vld1q_f32(partial_g.as_ptr().add(base));
            let bv = vld1q_f32(partial_b.as_ptr().add(base));
            acc_r = vfmaq_f32(acc_r, cy, rv);
            acc_g = vfmaq_f32(acc_g, cy, gv);
            acc_b = vfmaq_f32(acc_b, cy, bv);
        }

        // Extract lanes and convert to sRGB.
        let mut r_arr = [0.0f32; 4];
        let mut g_arr = [0.0f32; 4];
        let mut b_arr = [0.0f32; 4];
        vst1q_f32(r_arr.as_mut_ptr(), acc_r);
        vst1q_f32(g_arr.as_mut_ptr(), acc_g);
        vst1q_f32(b_arr.as_mut_ptr(), acc_b);

        for lane in 0..4 {
            let out_idx = (x + lane) * 3;
            *out_rgb.get_unchecked_mut(out_idx) = linear_to_srgb_fn(r_arr[lane]);
            *out_rgb.get_unchecked_mut(out_idx + 1) = linear_to_srgb_fn(g_arr[lane]);
            *out_rgb.get_unchecked_mut(out_idx + 2) = linear_to_srgb_fn(b_arr[lane]);
        }
    }

    // Scalar tail.
    for x in tail_start..width {
        let mut pr = 0.0f32;
        let mut pg = 0.0f32;
        let mut pb = 0.0f32;
        for j in 0..num_j {
            let cy = *cos_y_vals.get_unchecked(j);
            let idx = j * width + x;
            pr += cy * *partial_r.get_unchecked(idx);
            pg += cy * *partial_g.get_unchecked(idx);
            pb += cy * *partial_b.get_unchecked(idx);
        }
        let out_idx = x * 3;
        *out_rgb.get_unchecked_mut(out_idx) = linear_to_srgb_fn(pr);
        *out_rgb.get_unchecked_mut(out_idx + 1) = linear_to_srgb_fn(pg);
        *out_rgb.get_unchecked_mut(out_idx + 2) = linear_to_srgb_fn(pb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dot_product_basic() {
        let a = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        let b = vec![2.0f32, 3.0, 4.0, 5.0, 6.0];
        let result = dot_product_f32(&a, &b, 5);
        let expected: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        assert!((result - expected).abs() < 1e-5, "got {result}, expected {expected}");
    }

    #[test]
    fn test_dot_product_large() {
        let n = 512;
        let a: Vec<f32> = (0..n).map(|i| (i as f32 * 0.01).cos()).collect();
        let b: Vec<f32> = (0..n).map(|i| (i as f32 * 0.02).sin()).collect();
        let result = dot_product_f32(&a, &b, n);
        let expected: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        // Allow small FMA-related differences.
        assert!(
            (result - expected).abs() < 1e-2,
            "got {result}, expected {expected}"
        );
    }

    #[test]
    fn test_dot_product_3ch_basic() {
        let cos = vec![1.0f32, 2.0, 3.0, 4.0];
        let r = vec![0.1f32, 0.2, 0.3, 0.4];
        let g = vec![0.5f32, 0.6, 0.7, 0.8];
        let b = vec![0.9f32, 1.0, 1.1, 1.2];

        let (sr, sg, sb) = dot_product_3ch_f32(&cos, &r, &g, &b, 4);

        let er: f32 = cos.iter().zip(r.iter()).map(|(c, v)| c * v).sum();
        let eg: f32 = cos.iter().zip(g.iter()).map(|(c, v)| c * v).sum();
        let eb: f32 = cos.iter().zip(b.iter()).map(|(c, v)| c * v).sum();

        assert!((sr - er).abs() < 1e-5, "r: got {sr}, expected {er}");
        assert!((sg - eg).abs() < 1e-5, "g: got {sg}, expected {eg}");
        assert!((sb - eb).abs() < 1e-5, "b: got {sb}, expected {eb}");
    }

    #[test]
    fn test_dot_product_3ch_large() {
        let n = 256;
        let cos: Vec<f32> = (0..n).map(|i| (i as f32 * 0.01).cos()).collect();
        let r: Vec<f32> = (0..n).map(|i| (i as f32 * 0.005).sin()).collect();
        let g: Vec<f32> = (0..n).map(|i| (i as f32 * 0.007).cos()).collect();
        let b: Vec<f32> = (0..n).map(|i| (i as f32 * 0.003).sin()).collect();

        let (sr, sg, sb) = dot_product_3ch_f32(&cos, &r, &g, &b, n);

        let er: f32 = cos.iter().zip(r.iter()).map(|(c, v)| c * v).sum();
        let eg: f32 = cos.iter().zip(g.iter()).map(|(c, v)| c * v).sum();
        let eb: f32 = cos.iter().zip(b.iter()).map(|(c, v)| c * v).sum();

        assert!((sr - er).abs() < 1e-1, "r: got {sr}, expected {er}");
        assert!((sg - eg).abs() < 1e-1, "g: got {sg}, expected {eg}");
        assert!((sb - eb).abs() < 1e-1, "b: got {sb}, expected {eb}");
    }

    #[test]
    fn test_decode_accumulate_row_basic() {
        // 2 component rows, width=4
        let cos_y = vec![0.5f32, 1.0];
        let partial_r = vec![1.0f32, 2.0, 3.0, 4.0, 0.1, 0.2, 0.3, 0.4];
        let partial_g = vec![0.5f32, 0.6, 0.7, 0.8, 0.05, 0.06, 0.07, 0.08];
        let partial_b = vec![0.2f32, 0.3, 0.4, 0.5, 0.02, 0.03, 0.04, 0.05];

        let mut out = vec![0u8; 4 * 3];
        // Use a simple identity-ish conversion for testing.
        fn test_srgb(v: f32) -> u8 {
            (v.clamp(0.0, 1.0) * 255.0) as u8
        }

        decode_accumulate_row(
            &cos_y,
            &partial_r,
            &partial_g,
            &partial_b,
            4,
            2,
            &mut out,
            test_srgb,
        );

        // For x=0: r = 0.5*1.0 + 1.0*0.1 = 0.6 -> 153
        assert_eq!(out[0], (0.6f32 * 255.0) as u8);
    }
}
