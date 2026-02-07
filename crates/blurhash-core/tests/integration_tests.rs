use blurhash_core::{base83, components, decode, encode};

// ---------------------------------------------------------------------------
// Known test vectors
// ---------------------------------------------------------------------------

/// Reference blurhash from the official spec / woltapp README.
const KNOWN_HASH: &str = "LEHV6nWB2yk8pyo0adR*.7kCMdnj";

/// Helper: encode a solid white image to get the DC-only hash at runtime.
fn dc_only_white() -> String {
    let white_pixels = vec![255u8; 4 * 4 * 3];
    encode(&white_pixels, 4, 4, 1, 1).expect("encode white")
}

// ---------------------------------------------------------------------------
// Helper: generate a synthetic gradient image (row-major, RGB u8)
// ---------------------------------------------------------------------------
fn gradient_image(width: usize, height: usize) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            let r = ((x as f64 / width as f64) * 255.0) as u8;
            let g = ((y as f64 / height as f64) * 255.0) as u8;
            let b = 128u8;
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
        }
    }
    pixels
}

/// Generate a solid-colour image.
fn solid_image(width: usize, height: usize, r: u8, g: u8, b: u8) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width * height * 3);
    for _ in 0..(width * height) {
        pixels.push(r);
        pixels.push(g);
        pixels.push(b);
    }
    pixels
}

// ===========================================================================
// Base83 tests
// ===========================================================================

#[test]
fn base83_encode_zero() {
    assert_eq!(base83::encode(0, 1).unwrap(), "0");
    assert_eq!(base83::encode(0, 4).unwrap(), "0000");
}

#[test]
fn base83_encode_max_single_digit() {
    // 82 should be the last character in the alphabet: '~'
    assert_eq!(base83::encode(82, 1).unwrap(), "~");
}

#[test]
fn base83_encode_roundtrip() {
    for value in [0u64, 1, 42, 82, 83, 999, 6888, 83_u64.pow(4) - 1] {
        let len = if value == 0 {
            1
        } else {
            (value as f64).log(83.0).floor() as usize + 1
        };
        let encoded = base83::encode(value, len).expect("encode ok");
        let decoded = base83::decode(&encoded).expect("valid base83");
        assert_eq!(decoded, value, "roundtrip failed for {value}");
    }
}

#[test]
fn base83_decode_known() {
    // "10" in base83 = 1*83 + 0 = 83
    assert_eq!(base83::decode("10").unwrap(), 83);
}

#[test]
fn base83_decode_invalid_char() {
    assert!(base83::decode("!!!").is_err());
}

// ===========================================================================
// Component extraction
// ===========================================================================

#[test]
fn components_from_known_hash() {
    let (cx, cy) = components(KNOWN_HASH).expect("valid hash");
    assert_eq!(cx, 4);
    assert_eq!(cy, 3);
}

#[test]
fn components_1x1() {
    let (cx, cy) = components(&dc_only_white()).expect("valid hash");
    assert_eq!(cx, 1);
    assert_eq!(cy, 1);
}

#[test]
fn components_too_short() {
    assert!(components("ABCDE").is_err());
}

// ===========================================================================
// Decode tests
// ===========================================================================

#[test]
fn decode_known_hash_dimensions() {
    let pixels = decode(KNOWN_HASH, 32, 32, 1.0).expect("decode ok");
    assert_eq!(pixels.len(), 32 * 32 * 3);
}

#[test]
fn decode_known_hash_pixel_range() {
    let pixels = decode(KNOWN_HASH, 8, 8, 1.0).expect("decode ok");
    // Verify decode produced non-trivial output (not all zeros)
    assert!(pixels.iter().any(|&v| v > 0));
}

#[test]
fn decode_dc_only_white_is_white() {
    let pixels = decode(&dc_only_white(), 4, 4, 1.0).expect("decode ok");
    // Every pixel should be (255, 255, 255)
    for chunk in pixels.chunks(3) {
        assert!(
            chunk[0] >= 253 && chunk[1] >= 253 && chunk[2] >= 253,
            "expected near-white, got ({}, {}, {})",
            chunk[0],
            chunk[1],
            chunk[2]
        );
    }
}

#[test]
fn decode_invalid_length() {
    assert!(decode("LEHV6", 8, 8, 1.0).is_err());
}

#[test]
fn decode_invalid_characters() {
    // '!' is not in the base83 alphabet
    assert!(decode("!EHVWB2yk8pyo0adR*.7kCMdnj", 8, 8, 1.0).is_err());
}

#[test]
fn decode_mismatched_length() {
    // A valid first char that implies 4x3 components but truncated payload
    assert!(decode("LEHV6nWB", 8, 8, 1.0).is_err());
}

#[test]
fn decode_with_punch() {
    let normal = decode(KNOWN_HASH, 8, 8, 1.0).expect("decode ok");
    let punched = decode(KNOWN_HASH, 8, 8, 2.0).expect("decode ok");
    // Punched image should differ from normal (AC components are amplified)
    assert_ne!(normal, punched);
}

// ===========================================================================
// Encode tests
// ===========================================================================

#[test]
fn encode_gradient_4x3() {
    let img = gradient_image(32, 32);
    let hash = encode(&img, 32, 32, 4, 3).expect("encode ok");
    // Expected length: 4 + 2 * 4 * 3 = 28
    assert_eq!(hash.len(), 28);
}

#[test]
fn encode_1x1_components() {
    let img = solid_image(8, 8, 255, 255, 255);
    let hash = encode(&img, 8, 8, 1, 1).expect("encode ok");
    // 4 + 2*1*1 = 6
    assert_eq!(hash.len(), 6);
}

#[test]
fn encode_9x9_components() {
    let img = gradient_image(32, 32);
    let hash = encode(&img, 32, 32, 9, 9).expect("encode ok");
    // 4 + 2*9*9 = 166
    assert_eq!(hash.len(), 166);
}

#[test]
fn encode_invalid_components_zero() {
    let img = gradient_image(8, 8);
    assert!(encode(&img, 8, 8, 0, 4).is_err());
    assert!(encode(&img, 8, 8, 4, 0).is_err());
}

#[test]
fn encode_invalid_components_too_large() {
    let img = gradient_image(8, 8);
    assert!(encode(&img, 8, 8, 10, 4).is_err());
    assert!(encode(&img, 8, 8, 4, 10).is_err());
}

#[test]
fn encode_only_base83_chars() {
    let img = gradient_image(16, 16);
    let hash = encode(&img, 16, 16, 4, 4).expect("encode ok");
    let valid_chars: &str =
        "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz#$%*+,-.:;=?@[]^_{|}~";
    for ch in hash.chars() {
        assert!(
            valid_chars.contains(ch),
            "invalid base83 character in hash: '{ch}'"
        );
    }
}

// ===========================================================================
// Encode -> Decode round-trip
// ===========================================================================

#[test]
fn roundtrip_solid_red() {
    let img = solid_image(16, 16, 255, 0, 0);
    let hash = encode(&img, 16, 16, 4, 4).expect("encode ok");
    let decoded = decode(&hash, 16, 16, 1.0).expect("decode ok");
    // The DC component should dominate, giving us a reddish average
    let avg_r: f64 = decoded.chunks(3).map(|c| c[0] as f64).sum::<f64>() / (16.0 * 16.0);
    let avg_g: f64 = decoded.chunks(3).map(|c| c[1] as f64).sum::<f64>() / (16.0 * 16.0);
    let avg_b: f64 = decoded.chunks(3).map(|c| c[2] as f64).sum::<f64>() / (16.0 * 16.0);
    assert!(avg_r > 200.0, "avg red = {avg_r}, expected > 200");
    assert!(avg_g < 80.0, "avg green = {avg_g}, expected < 80");
    assert!(avg_b < 80.0, "avg blue = {avg_b}, expected < 80");
}

#[test]
fn roundtrip_solid_green() {
    let img = solid_image(16, 16, 0, 255, 0);
    let hash = encode(&img, 16, 16, 4, 4).expect("encode ok");
    let decoded = decode(&hash, 16, 16, 1.0).expect("decode ok");
    let avg_r: f64 = decoded.chunks(3).map(|c| c[0] as f64).sum::<f64>() / (16.0 * 16.0);
    let avg_g: f64 = decoded.chunks(3).map(|c| c[1] as f64).sum::<f64>() / (16.0 * 16.0);
    let avg_b: f64 = decoded.chunks(3).map(|c| c[2] as f64).sum::<f64>() / (16.0 * 16.0);
    assert!(avg_r < 80.0, "avg red = {avg_r}, expected < 80");
    assert!(avg_g > 200.0, "avg green = {avg_g}, expected > 200");
    assert!(avg_b < 80.0, "avg blue = {avg_b}, expected < 80");
}

#[test]
fn roundtrip_solid_blue() {
    let img = solid_image(16, 16, 0, 0, 255);
    let hash = encode(&img, 16, 16, 4, 4).expect("encode ok");
    let decoded = decode(&hash, 16, 16, 1.0).expect("decode ok");
    let avg_r: f64 = decoded.chunks(3).map(|c| c[0] as f64).sum::<f64>() / (16.0 * 16.0);
    let avg_g: f64 = decoded.chunks(3).map(|c| c[1] as f64).sum::<f64>() / (16.0 * 16.0);
    let avg_b: f64 = decoded.chunks(3).map(|c| c[2] as f64).sum::<f64>() / (16.0 * 16.0);
    assert!(avg_r < 80.0, "avg red = {avg_r}, expected < 80");
    assert!(avg_g < 80.0, "avg green = {avg_g}, expected < 80");
    assert!(avg_b > 200.0, "avg blue = {avg_b}, expected > 200");
}

#[test]
fn roundtrip_gradient_preserves_components() {
    let img = gradient_image(32, 32);
    let hash = encode(&img, 32, 32, 4, 3).expect("encode ok");
    let (cx, cy) = components(&hash).expect("valid hash");
    assert_eq!(cx, 4);
    assert_eq!(cy, 3);
}

#[test]
fn roundtrip_deterministic() {
    let img = gradient_image(16, 16);
    let hash1 = encode(&img, 16, 16, 4, 4).expect("encode ok");
    let hash2 = encode(&img, 16, 16, 4, 4).expect("encode ok");
    assert_eq!(hash1, hash2, "encoding should be deterministic");
}

// ===========================================================================
// Edge cases
// ===========================================================================

#[test]
fn encode_small_1x1_image() {
    let img = vec![128u8, 64, 32];
    let hash = encode(&img, 1, 1, 1, 1).expect("encode ok");
    assert_eq!(hash.len(), 6);
}

#[test]
fn encode_large_image_256x256() {
    let img = gradient_image(256, 256);
    let hash = encode(&img, 256, 256, 4, 4).expect("encode ok");
    assert_eq!(hash.len(), 4 + 2 * 4 * 4);
}

#[test]
fn decode_to_small_1x1() {
    let pixels = decode(KNOWN_HASH, 1, 1, 1.0).expect("decode ok");
    assert_eq!(pixels.len(), 3);
}

#[test]
fn decode_to_large_256x256() {
    let pixels = decode(KNOWN_HASH, 256, 256, 1.0).expect("decode ok");
    assert_eq!(pixels.len(), 256 * 256 * 3);
}

#[test]
fn roundtrip_various_component_counts() {
    let img = gradient_image(32, 32);
    for cx in 1..=9 {
        for cy in 1..=9 {
            let hash = encode(&img, 32, 32, cx, cy)
                .unwrap_or_else(|e| panic!("encode failed for {cx}x{cy}: {e}"));
            let expected_len = 4 + 2 * cx as usize * cy as usize;
            assert_eq!(hash.len(), expected_len, "wrong hash length for {cx}x{cy}");
            let (rcx, rcy) = components(&hash).unwrap();
            assert_eq!(rcx, cx);
            assert_eq!(rcy, cy);
            let pixels = decode(&hash, 8, 8, 1.0)
                .unwrap_or_else(|e| panic!("decode failed for {cx}x{cy}: {e}"));
            assert_eq!(pixels.len(), 8 * 8 * 3);
        }
    }
}

#[test]
fn roundtrip_non_square_image() {
    let img = gradient_image(64, 16);
    let hash = encode(&img, 64, 16, 5, 2).expect("encode ok");
    let decoded = decode(&hash, 64, 16, 1.0).expect("decode ok");
    assert_eq!(decoded.len(), 64 * 16 * 3);
}

// ===========================================================================
// sRGB / linear conversion consistency
// ===========================================================================

#[test]
fn srgb_linear_roundtrip() {
    // Encode a ramp image, decode it; the DC value for a uniform image
    // should reconstruct the original colour closely.
    for val in [0u8, 1, 50, 128, 200, 254, 255] {
        let img = solid_image(4, 4, val, val, val);
        let hash = encode(&img, 4, 4, 1, 1).expect("encode ok");
        let decoded = decode(&hash, 1, 1, 1.0).expect("decode ok");
        let diff = (decoded[0] as i16 - val as i16).unsigned_abs();
        assert!(
            diff <= 1,
            "sRGB roundtrip failed for {val}: got {}, diff {diff}",
            decoded[0]
        );
    }
}
