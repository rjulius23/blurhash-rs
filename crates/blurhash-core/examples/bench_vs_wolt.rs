//! Benchmark comparison: blurhash-rs vs Wolt blurhash (TypeScript)
//! Run with: cargo run -p blurhash-core --example bench_vs_wolt --release

use blurhash_core::{decode, encode};
use std::time::Instant;

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

fn bench_encode(pixels: &[u8], w: u32, h: u32, cx: u32, cy: u32, iters: u32) -> f64 {
    let _ = encode(pixels, w, h, cx, cy).unwrap();
    let start = Instant::now();
    for _ in 0..iters {
        let _ = encode(pixels, w, h, cx, cy).unwrap();
    }
    start.elapsed().as_secs_f64() * 1000.0 / iters as f64
}

fn bench_decode(hash: &str, w: u32, h: u32, iters: u32) -> f64 {
    let _ = decode(hash, w, h, 1.0).unwrap();
    let start = Instant::now();
    for _ in 0..iters {
        let _ = decode(hash, w, h, 1.0).unwrap();
    }
    start.elapsed().as_secs_f64() * 1000.0 / iters as f64
}

fn main() {
    println!("=== blurhash-rs (Rust) Benchmark ===\n");

    // Encode 4x3
    println!("--- ENCODE (4x3 components) ---");
    for &(w, h) in &[(32u32, 32), (128, 128), (256, 256)] {
        let img = gradient_image(w as usize, h as usize);
        let iters = if w <= 32 { 1000 } else if w <= 128 { 100 } else { 50 };
        let ms = bench_encode(&img, w, h, 4, 3, iters);
        println!("  encode {}x{}: {:.4} ms", w, h, ms);
    }

    // Luki-web: 200x200, 3x9
    let img200 = gradient_image(200, 200);
    let ms = bench_encode(&img200, 200, 200, 3, 9, 50);
    println!("  encode 200x200 (3x9, luki-web): {:.4} ms", ms);

    // Large 9x9
    println!("\n--- ENCODE (9x9 components, large) ---");
    for &(w, h) in &[(512u32, 512), (1024, 1024)] {
        let img = gradient_image(w as usize, h as usize);
        let iters = if w <= 512 { 10 } else { 3 };
        let ms = bench_encode(&img, w, h, 9, 9, iters);
        println!("  encode {}x{} 9x9: {:.2} ms", w, h, ms);
    }

    // Decode
    println!("\n--- DECODE ---");
    let small = gradient_image(32, 32);
    let hash43 = encode(&small, 32, 32, 4, 3).unwrap();
    let hash99 = encode(&small, 32, 32, 9, 9).unwrap();

    for &(w, h) in &[(32u32, 32), (128, 128), (256, 256)] {
        let iters = if w <= 32 { 1000 } else if w <= 128 { 100 } else { 50 };
        let ms = bench_decode(&hash43, w, h, iters);
        println!("  decode {}x{} (4x3): {:.4} ms", w, h, ms);
    }

    // Luki-web decode: 400x300, 3x9
    let hash_luki = encode(&small, 32, 32, 3, 9).unwrap();
    let ms = bench_decode(&hash_luki, 400, 300, 50);
    println!("  decode 400x300 (3x9, luki-web): {:.4} ms", ms);

    println!("\nDone.");
}
