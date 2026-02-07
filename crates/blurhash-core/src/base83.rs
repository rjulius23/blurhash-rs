//! Base83 encoding and decoding used by the BlurHash algorithm.
//!
//! The BlurHash specification uses a custom base83 encoding with a specific
//! 83-character alphabet. This module provides functions to encode integers
//! into base83 strings and decode base83 strings back into integers.

use crate::error::BlurhashError;

/// The 83-character alphabet used by BlurHash base83 encoding.
const ALPHABET: &[u8; 83] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz#$%*+,-.:;=?@[]^_{|}~";

/// Lookup table mapping ASCII byte values to their base83 digit value.
/// Invalid characters map to `255`.
const fn build_decode_lut() -> [u8; 128] {
    let mut lut = [255u8; 128];
    let mut i = 0;
    while i < 83 {
        lut[ALPHABET[i] as usize] = i as u8;
        i += 1;
    }
    lut
}

/// Precomputed decode lookup table (computed at compile time).
static DECODE_LUT: [u8; 128] = build_decode_lut();

/// Decode a base83 string into an integer.
///
/// # Errors
///
/// Returns [`BlurhashError::InvalidBase83Character`] if the string contains
/// a character not in the base83 alphabet.
///
/// # Examples
///
/// ```
/// use blurhash_core::base83::decode;
/// assert_eq!(decode("0").unwrap(), 0);
/// assert_eq!(decode("~").unwrap(), 82);
/// ```
pub fn decode(base83_str: &str) -> Result<u64, BlurhashError> {
    let mut value: u64 = 0;
    for ch in base83_str.bytes() {
        if ch >= 128 {
            return Err(BlurhashError::InvalidBase83Character(ch as char));
        }
        let digit = DECODE_LUT[ch as usize];
        if digit == 255 {
            return Err(BlurhashError::InvalidBase83Character(ch as char));
        }
        value = value
            .checked_mul(83)
            .and_then(|v| v.checked_add(digit as u64))
            .ok_or_else(|| {
                BlurhashError::EncodingError(format!(
                    "base83 value overflow decoding {:?}",
                    base83_str
                ))
            })?;
    }
    Ok(value)
}

/// Encode an integer into a base83 string of the specified length.
///
/// # Errors
///
/// Returns [`BlurhashError::EncodingError`] if the value is too large to
/// be represented in the given number of digits.
///
/// # Examples
///
/// ```
/// use blurhash_core::base83::encode;
/// assert_eq!(encode(0, 1).unwrap(), "0");
/// assert_eq!(encode(82, 1).unwrap(), "~");
/// ```
pub fn encode(value: u64, length: usize) -> Result<String, BlurhashError> {
    // Check that the value fits in the specified length.
    // 83^length is the first value that does NOT fit.
    let max_value = 83u64.checked_pow(length as u32).unwrap_or(u64::MAX);
    if value >= max_value {
        return Err(BlurhashError::EncodingError(format!(
            "value {value} is too large for {length} base83 digits (max {})",
            max_value - 1
        )));
    }

    let mut result = vec![0u8; length];
    let mut remaining = value;
    for i in (0..length).rev() {
        let digit = (remaining % 83) as usize;
        remaining /= 83;
        result[i] = ALPHABET[digit];
    }
    // SAFETY: all bytes come from ALPHABET which is valid ASCII, so this cannot fail.
    Ok(unsafe { String::from_utf8_unchecked(result) })
}

/// Encode an integer into a base83 string, writing directly into a byte buffer.
///
/// Writes `length` ASCII bytes starting at `buf[offset]`. Returns the new
/// offset (`offset + length`) for chaining.
///
/// # Panics
///
/// Panics in debug mode if the buffer is too small or the value overflows.
#[inline(always)]
pub(crate) fn encode_to_buf(value: u64, length: usize, buf: &mut [u8], offset: usize) -> usize {
    debug_assert!(
        offset + length <= buf.len(),
        "base83 encode_to_buf: buffer too small"
    );
    // Specialized paths for the exact lengths used in BlurHash encoding.
    // These avoid the loop and let the compiler optimize to straight-line code.
    unsafe {
        match length {
            1 => {
                debug_assert!(value < 83);
                *buf.get_unchecked_mut(offset) = *ALPHABET.get_unchecked(value as usize);
            }
            2 => {
                debug_assert!(value < 83 * 83);
                let d1 = (value % 83) as usize;
                let d0 = (value / 83) as usize;
                *buf.get_unchecked_mut(offset) = *ALPHABET.get_unchecked(d0);
                *buf.get_unchecked_mut(offset + 1) = *ALPHABET.get_unchecked(d1);
            }
            4 => {
                debug_assert!(value < 83 * 83 * 83 * 83);
                let d3 = (value % 83) as usize;
                let rem = value / 83;
                let d2 = (rem % 83) as usize;
                let rem = rem / 83;
                let d1 = (rem % 83) as usize;
                let d0 = (rem / 83) as usize;
                *buf.get_unchecked_mut(offset) = *ALPHABET.get_unchecked(d0);
                *buf.get_unchecked_mut(offset + 1) = *ALPHABET.get_unchecked(d1);
                *buf.get_unchecked_mut(offset + 2) = *ALPHABET.get_unchecked(d2);
                *buf.get_unchecked_mut(offset + 3) = *ALPHABET.get_unchecked(d3);
            }
            _ => {
                let mut remaining = value;
                for i in (offset..offset + length).rev() {
                    let digit = (remaining % 83) as usize;
                    remaining /= 83;
                    *buf.get_unchecked_mut(i) = *ALPHABET.get_unchecked(digit);
                }
            }
        }
    }
    offset + length
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_single_chars() {
        assert_eq!(decode("0").unwrap(), 0);
        assert_eq!(decode("1").unwrap(), 1);
        assert_eq!(decode("~").unwrap(), 82);
    }

    #[test]
    fn test_encode_single_chars() {
        assert_eq!(encode(0, 1).unwrap(), "0");
        assert_eq!(encode(1, 1).unwrap(), "1");
        assert_eq!(encode(82, 1).unwrap(), "~");
    }

    #[test]
    fn test_roundtrip() {
        for value in [0, 1, 42, 82, 83, 100, 1000, 83 * 83 - 1, 83 * 83 * 83 - 1] {
            let length = if value == 0 {
                1
            } else {
                ((value as f64).log(83.0).floor() as usize) + 1
            };
            let encoded = encode(value, length).unwrap();
            let decoded = decode(&encoded).unwrap();
            assert_eq!(decoded, value, "roundtrip failed for {value}");
        }
    }

    #[test]
    fn test_encode_with_padding() {
        // Encoding 0 with length 4 should give "0000"
        assert_eq!(encode(0, 4).unwrap(), "0000");
        // Encoding 1 with length 4 should give "0001"
        assert_eq!(encode(1, 4).unwrap(), "0001");
    }

    #[test]
    fn test_decode_multi_char() {
        // "10" in base83 = 1*83 + 0 = 83
        assert_eq!(decode("10").unwrap(), 83);
        // "00" = 0
        assert_eq!(decode("00").unwrap(), 0);
    }

    #[test]
    fn test_decode_invalid_char() {
        assert!(decode(" ").is_err());
        assert!(decode("!").is_err());
    }

    #[test]
    fn test_encode_value_too_large() {
        assert!(encode(83, 1).is_err());
        assert!(encode(83 * 83, 2).is_err());
    }

    #[test]
    fn test_alphabet_completeness() {
        // Every character in the alphabet should decode to its index
        for (i, &ch) in ALPHABET.iter().enumerate() {
            let s = String::from(ch as char);
            assert_eq!(decode(&s).unwrap(), i as u64);
        }
    }

    #[test]
    fn test_encode_to_buf_matches_encode() {
        let mut buf = [0u8; 16];
        for &(value, length) in &[(0u64, 1), (82, 1), (0, 4), (1, 4), (83 * 83 - 1, 2)] {
            let expected = encode(value, length).unwrap();
            let end = encode_to_buf(value, length, &mut buf, 0);
            assert_eq!(end, length);
            assert_eq!(
                std::str::from_utf8(&buf[..length]).unwrap(),
                expected.as_str(),
                "mismatch for value={value}, length={length}"
            );
        }
    }

    #[test]
    fn test_encode_to_buf_chaining() {
        let mut buf = [0u8; 8];
        let mut offset = 0;
        offset = encode_to_buf(21, 1, &mut buf, offset); // size flag
        offset = encode_to_buf(5, 1, &mut buf, offset); // quant_max_ac
        offset = encode_to_buf(123456, 4, &mut buf, offset); // DC
        assert_eq!(offset, 6);
        // Verify each segment matches individual encode
        assert_eq!(
            std::str::from_utf8(&buf[0..1]).unwrap(),
            encode(21, 1).unwrap()
        );
        assert_eq!(
            std::str::from_utf8(&buf[1..2]).unwrap(),
            encode(5, 1).unwrap()
        );
        assert_eq!(
            std::str::from_utf8(&buf[2..6]).unwrap(),
            encode(123456, 4).unwrap()
        );
    }
}
