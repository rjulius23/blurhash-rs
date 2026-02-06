//! BlurHash Demo - Encode and decode example
//!
//! Run with: cargo run --example demo

use blurhash_core::{decode, encode, components};

fn main() {
    println!("=== BlurHash Demo ===\n");

    // Create a simple 4x4 gradient image (RGB)
    // Top-left: red, Top-right: green, Bottom-left: blue, Bottom-right: white
    let width = 4;
    let height = 4;
    let mut pixels = Vec::with_capacity(width * height * 3);

    for y in 0..height {
        for x in 0..width {
            let r = ((x as f32 / (width - 1) as f32) * 255.0) as u8;
            let g = ((y as f32 / (height - 1) as f32) * 255.0) as u8;
            let b = 128;
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
        }
    }

    println!("1. Created a {}x{} gradient image", width, height);
    println!("   Pixels (first 12 bytes): {:?}...\n", &pixels[..12]);

    // Encode to BlurHash
    let components_x = 4;
    let components_y = 3;
    let hash = encode(&pixels, width as u32, height as u32, components_x, components_y)
        .expect("Failed to encode");

    println!("2. Encoded to BlurHash: {}", hash);
    println!("   Components: {}x{}", components_x, components_y);
    println!("   Hash length: {} characters\n", hash.len());

    // Extract components from hash
    let (cx, cy) = components(&hash).expect("Failed to get components");
    println!("3. Extracted components from hash: {}x{}\n", cx, cy);

    // Decode back to pixels
    let decode_width = 8;
    let decode_height = 8;
    let punch = 1.0; // Normal contrast
    let decoded = decode(&hash, decode_width, decode_height, punch)
        .expect("Failed to decode");

    println!("4. Decoded to {}x{} image ({} bytes)",
             decode_width, decode_height, decoded.len());
    println!("   First pixel RGB: ({}, {}, {})",
             decoded[0], decoded[1], decoded[2]);
    println!("   Last pixel RGB: ({}, {}, {})",
             decoded[decoded.len()-3], decoded[decoded.len()-2], decoded[decoded.len()-1]);

    println!("\n=== Demo Complete ===");
    println!("\nTry encoding your own images!");
    println!("See README.md for Python and TypeScript usage.");
}
