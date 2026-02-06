//! Color space conversion utilities for sRGB and linear RGB.
//!
//! Provides fast sRGB-to-linear and linear-to-sRGB conversions using a
//! precomputed lookup table for the sRGB-to-linear direction.

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
            // (value + 0.055) / 1.055 raised to 2.4
            // const fn doesn't support f64::powf, so we use an approximation via exp/ln.
            // We'll fill this in at runtime initialization instead.
            // Actually, we can compute it with a helper.
            const_powf((value + 0.055) / 1.055, 2.4)
        };
        i += 1;
    }
    lut
}

/// Approximate f64::powf for const context using iterative exp-by-squaring
/// where exponent is 2.4 = 2 + 0.4 = 2 + 2/5.
/// We use the identity: x^2.4 = x^2 * x^(2/5) = x^2 * (x^2)^(1/5).
/// For the fifth root we use Newton's method.
const fn const_powf(base: f64, _exp: f64) -> f64 {
    // We know exp is always 2.4 for our use case.
    // x^2.4 = x^2 * x^0.4 = x^2 * (x^2)^0.2 = x^2 * fifth_root(x^2)
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

/// Precomputed sRGB-to-linear lookup table.
static SRGB_TO_LINEAR_LUT: [f64; 256] = build_srgb_to_linear_lut();

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

/// Convert a linear RGB value (0.0..=1.0) to an sRGB byte value (0..=255).
///
/// Values outside \[0.0, 1.0\] are clamped.
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
    if clamped <= 0.003_130_8 {
        (clamped * 12.92 * 255.0 + 0.5) as u8
    } else {
        ((1.055 * clamped.powf(1.0 / 2.4) - 0.055) * 255.0 + 0.5) as u8
    }
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
}
