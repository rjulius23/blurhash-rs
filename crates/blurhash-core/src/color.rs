//! Color space conversion utilities for sRGB and linear RGB.
//!
//! Provides fast sRGB-to-linear and linear-to-sRGB conversions using
//! precomputed lookup tables for both directions.

/// Precomputed lookup table mapping sRGB byte values (0..=255) to linear RGB (0.0..=1.0).
/// Computed at compile time for zero runtime cost.
const fn build_srgb_to_linear_lut() -> [f64; 256] {
    let mut lut = [0.0f64; 256];
    let mut i = 0u32;
    while i < 256 {
        let value = i as f64 / 255.0;
        lut[i as usize] = if value <= 0.04045 {
            value / 12.92
        } else {
            const_powf((value + 0.055) / 1.055, 2.4)
        };
        i += 1;
    }
    lut
}

/// Build f32 version of sRGB-to-linear LUT for use in optimized paths.
const fn build_srgb_to_linear_lut_f32() -> [f32; 256] {
    let lut64 = build_srgb_to_linear_lut();
    let mut lut = [0.0f32; 256];
    let mut i = 0;
    while i < 256 {
        lut[i] = lut64[i] as f32;
        i += 1;
    }
    lut
}

/// Compute `base^2.4` in const context using the identity
/// `x^2.4 = x^2 * (x^2)^(1/5)`, where the fifth root is computed via
/// Newton's method (converges to within 1e-15).
///
/// The `_exp` parameter is ignored; this function always computes `base^2.4`.
const fn const_powf(base: f64, _exp: f64) -> f64 {
    if base <= 0.0 {
        return 0.0;
    }
    let x2 = base * base;
    let fifth_root = const_nth_root(x2, 5);
    x2 * fifth_root
}

/// Compute the nth root of `value` using Newton's method in const context.
const fn const_nth_root(value: f64, n: u32) -> f64 {
    if value <= 0.0 {
        return 0.0;
    }
    if value == 1.0 {
        return 1.0;
    }
    // Newton's method: x_{k+1} = ((n-1)*x_k + value / x_k^(n-1)) / n
    let mut x = value; // initial guess
    if value < 1.0 {
        x = 1.0; // better starting guess for values < 1
    }
    let nf = n as f64;
    let nm1 = (n - 1) as f64;
    let mut i = 0;
    while i < 100 {
        // Compute x^(n-1)
        let mut xpow = 1.0;
        let mut j = 0;
        while j < n - 1 {
            xpow *= x;
            j += 1;
        }
        let x_new = (nm1 * x + value / xpow) / nf;
        // Check convergence
        let diff = if x_new > x { x_new - x } else { x - x_new };
        if diff < 1e-15 {
            return x_new;
        }
        x = x_new;
        i += 1;
    }
    x
}

/// Precomputed sRGB-to-linear lookup table (f64).
static SRGB_TO_LINEAR_LUT: [f64; 256] = build_srgb_to_linear_lut();

/// Precomputed sRGB-to-linear lookup table (f32).
static SRGB_TO_LINEAR_LUT_F32: [f32; 256] = build_srgb_to_linear_lut_f32();

/// Convert an sRGB byte value (0..=255) to linear RGB (0.0..=1.0).
///
/// Uses a precomputed lookup table for O(1) performance.
///
/// # Examples
///
/// ```
/// use blurhash_core::color::srgb_to_linear;
/// assert!((srgb_to_linear(0) - 0.0).abs() < 1e-10);
/// assert!((srgb_to_linear(255) - 1.0).abs() < 1e-10);
/// ```
#[inline]
pub fn srgb_to_linear(value: u8) -> f64 {
    SRGB_TO_LINEAR_LUT[value as usize]
}

/// Convert an sRGB byte value (0..=255) to linear RGB as f32 (0.0..=1.0).
///
/// Uses a precomputed lookup table for O(1) performance.
#[inline]
pub fn srgb_to_linear_f32(value: u8) -> f32 {
    // SAFETY: value is u8, always in 0..256, so index is always valid.
    unsafe { *SRGB_TO_LINEAR_LUT_F32.get_unchecked(value as usize) }
}

/// Size of the linear-to-sRGB lookup table. 4096 entries provide sufficient
/// precision (12-bit quantization of the linear range) for exact byte-level
/// accuracy while keeping the table at only 4 KiB.
const LINEAR_TO_SRGB_LUT_SIZE: usize = 4096;

/// Compute a single linear-to-sRGB conversion using the exact formula.
/// Used only at compile time to build the LUT.
const fn linear_to_srgb_exact(linear: f64) -> u8 {
    if linear <= 0.0 {
        return 0;
    }
    if linear >= 1.0 {
        return 255;
    }
    if linear <= 0.003_130_8 {
        // The linear region: sRGB = linear * 12.92
        // We add 0.5 for rounding: (linear * 12.92 * 255.0 + 0.5) as u8
        let val = linear * 12.92 * 255.0 + 0.5;
        return val as u8;
    }
    // The gamma region: sRGB = 1.055 * linear^(1/2.4) - 0.055
    // 1/2.4 = 5/12, so linear^(5/12) = twelfth_root(linear^5)
    let l5 = linear * linear * linear * linear * linear;
    let root = const_nth_root(l5, 12);
    let val = (1.055 * root - 0.055) * 255.0 + 0.5;
    val as u8
}

/// Precomputed lookup table mapping quantized linear values to sRGB bytes.
const fn build_linear_to_srgb_lut() -> [u8; LINEAR_TO_SRGB_LUT_SIZE] {
    let mut lut = [0u8; LINEAR_TO_SRGB_LUT_SIZE];
    let mut i = 0u32;
    while i < LINEAR_TO_SRGB_LUT_SIZE as u32 {
        let linear = i as f64 / (LINEAR_TO_SRGB_LUT_SIZE as f64 - 1.0);
        lut[i as usize] = linear_to_srgb_exact(linear);
        i += 1;
    }
    lut
}

/// Precomputed linear-to-sRGB lookup table.
static LINEAR_TO_SRGB_LUT: [u8; LINEAR_TO_SRGB_LUT_SIZE] = build_linear_to_srgb_lut();

/// Convert a linear RGB value (0.0..=1.0) to an sRGB byte value (0..=255).
///
/// Uses a precomputed lookup table for O(1) performance. Values outside
/// \[0.0, 1.0\] are clamped.
///
/// # Examples
///
/// ```
/// use blurhash_core::color::linear_to_srgb;
/// assert_eq!(linear_to_srgb(0.0), 0);
/// assert_eq!(linear_to_srgb(1.0), 255);
/// ```
#[inline]
pub fn linear_to_srgb(value: f64) -> u8 {
    let clamped = value.clamp(0.0, 1.0);
    let index = (clamped * (LINEAR_TO_SRGB_LUT_SIZE as f64 - 1.0) + 0.5) as usize;
    // index is at most LINEAR_TO_SRGB_LUT_SIZE - 1 thanks to the clamp above, but
    // use min to keep the bounds check elided by the compiler.
    LINEAR_TO_SRGB_LUT[index.min(LINEAR_TO_SRGB_LUT_SIZE - 1)]
}

/// Convert a linear RGB f32 value to an sRGB byte value (0..=255).
///
/// Uses the precomputed lookup table. Values outside [0.0, 1.0] are clamped.
#[inline]
pub fn linear_to_srgb_f32(value: f32) -> u8 {
    let clamped = value.clamp(0.0, 1.0);
    let index = (clamped * (LINEAR_TO_SRGB_LUT_SIZE as f32 - 1.0) + 0.5) as usize;
    // SAFETY: clamped is in [0.0, 1.0], so index is in [0, LINEAR_TO_SRGB_LUT_SIZE-1].
    // The +0.5 rounding and clamp guarantee the index is in-bounds.
    unsafe { *LINEAR_TO_SRGB_LUT.get_unchecked(index.min(LINEAR_TO_SRGB_LUT_SIZE - 1)) }
}

/// Compute `sign(value) * |value|^exp`.
///
/// This preserves the sign of the input while raising the absolute value
/// to the given exponent.
///
/// # Examples
///
/// ```
/// use blurhash_core::color::sign_pow;
/// assert!((sign_pow(4.0, 0.5) - 2.0).abs() < 1e-10);
/// assert!((sign_pow(-4.0, 0.5) - (-2.0)).abs() < 1e-10);
/// ```
#[inline]
pub fn sign_pow(value: f64, exp: f64) -> f64 {
    value.abs().powf(exp).copysign(value)
}

/// Fast sign_pow for f32. Uses specialized paths for common exponents.
#[inline(always)]
pub fn sign_pow_f32(value: f32, exp: f32) -> f32 {
    let abs_val = value.abs();
    let result = if exp == 0.5 {
        abs_val.sqrt()
    } else if exp == 2.0 {
        abs_val * abs_val
    } else {
        // For the general case, use f32 powf.
        abs_val.powf(exp)
    };
    result.copysign(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srgb_to_linear_boundary() {
        assert!((srgb_to_linear(0) - 0.0).abs() < 1e-10);
        assert!((srgb_to_linear(255) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_srgb_to_linear_known_values() {
        // sRGB 128 should be approximately 0.2158605
        let val = srgb_to_linear(128);
        assert!((val - 0.2158605).abs() < 1e-4, "got {val}");
    }

    #[test]
    fn test_linear_to_srgb_boundary() {
        assert_eq!(linear_to_srgb(0.0), 0);
        assert_eq!(linear_to_srgb(1.0), 255);
    }

    #[test]
    fn test_linear_to_srgb_clamp() {
        assert_eq!(linear_to_srgb(-0.5), 0);
        assert_eq!(linear_to_srgb(1.5), 255);
    }

    #[test]
    fn test_roundtrip_srgb() {
        // Roundtrip: sRGB -> linear -> sRGB should be identity (within rounding)
        for i in 0..=255u8 {
            let linear = srgb_to_linear(i);
            let back = linear_to_srgb(linear);
            assert!(
                (i as i16 - back as i16).unsigned_abs() <= 1,
                "roundtrip failed for {i}: got {back}"
            );
        }
    }

    #[test]
    fn test_sign_pow_positive() {
        assert!((sign_pow(4.0, 0.5) - 2.0).abs() < 1e-10);
        assert!((sign_pow(9.0, 0.5) - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_sign_pow_negative() {
        assert!((sign_pow(-4.0, 0.5) - (-2.0)).abs() < 1e-10);
        assert!((sign_pow(-9.0, 0.5) - (-3.0)).abs() < 1e-10);
    }

    #[test]
    fn test_sign_pow_zero() {
        assert!((sign_pow(0.0, 2.0) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_linear_to_srgb_threshold() {
        // The threshold 0.0031308 should be handled correctly
        let below = linear_to_srgb(0.003);
        let above = linear_to_srgb(0.004);
        assert!(below < above);
    }

    #[test]
    fn test_srgb_to_linear_monotonic() {
        let mut prev = srgb_to_linear(0);
        for i in 1..=255u8 {
            let curr = srgb_to_linear(i);
            assert!(curr > prev, "not monotonic at {i}: {prev} >= {curr}");
            prev = curr;
        }
    }

    #[test]
    fn test_f32_roundtrip_srgb() {
        for i in 0..=255u8 {
            let linear = srgb_to_linear_f32(i);
            let back = linear_to_srgb_f32(linear);
            assert!(
                (i as i16 - back as i16).unsigned_abs() <= 1,
                "f32 roundtrip failed for {i}: got {back}"
            );
        }
    }

    #[test]
    fn test_sign_pow_f32() {
        assert!((sign_pow_f32(4.0, 0.5) - 2.0).abs() < 1e-5);
        assert!((sign_pow_f32(-4.0, 0.5) - (-2.0)).abs() < 1e-5);
        assert!((sign_pow_f32(3.0, 2.0) - 9.0).abs() < 1e-5);
        assert!((sign_pow_f32(-3.0, 2.0) - (-9.0)).abs() < 1e-5);
    }
}
