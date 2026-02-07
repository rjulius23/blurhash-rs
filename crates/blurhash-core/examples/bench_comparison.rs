//! Performance comparison: blurhash-rs vs MetalBlurHash
//!
//! Run with: cargo run -p blurhash-core --example bench_comparison --release

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

fn main() {
    println!("=== blurhash-rs vs MetalBlurHash Comparison ===");
    println!("Hardware: Apple Silicon (single-threaded CPU, no GPU)\n");

    // MetalBlurHash benchmark: encode 3648x5472, 9x9 components
    println!("--- ENCODE (3648x5472, 9x9 components) ---");
    let img = gradient_image(3648, 5472);

    // Warmup
    let _ = encode(&img, 3648, 5472, 9, 9).unwrap();

    let runs = 3;
    let mut total = std::time::Duration::ZERO;
    for _ in 0..runs {
        let start = Instant::now();
        let _ = encode(&img, 3648, 5472, 9, 9).unwrap();
        total += start.elapsed();
    }
    let avg_encode = total / runs;
    println!("  blurhash-rs (CPU):      {:.3}s", avg_encode.as_secs_f64());
    println!("  MetalBlurHash (GPU):    0.154s  (M1 Max)");
    println!("  Swift CPU original:     32.212s (M1 Max)");

    // MetalBlurHash benchmark: decode 3840x2560, 9x9 components
    println!("\n--- DECODE (3840x2560, 9x9 components) ---");
    let small = gradient_image(64, 64);
    let hash = encode(&small, 64, 64, 9, 9).unwrap();

    // Warmup
    let _ = decode(&hash, 3840, 2560, 1.0).unwrap();

    let mut total = std::time::Duration::ZERO;
    for _ in 0..runs {
        let start = Instant::now();
        let _ = decode(&hash, 3840, 2560, 1.0).unwrap();
        total += start.elapsed();
    }
    let avg_decode = total / runs;
    println!("  blurhash-rs (CPU):      {:.3}s", avg_decode.as_secs_f64());
    println!("  MetalBlurHash (GPU):    0.013s  (M1 Max)");
    println!("  Swift CPU original:     3.267s  (M1 Max)");

    // Summary
    println!("\n--- SUMMARY ---");
    let encode_vs_swift = 32.212 / avg_encode.as_secs_f64();
    let decode_vs_swift = 3.267 / avg_decode.as_secs_f64();
    let encode_vs_metal = avg_encode.as_secs_f64() / 0.154;
    let decode_vs_metal = avg_decode.as_secs_f64() / 0.013;

    println!(
        "  vs Swift CPU:  encode {:.0}x faster, decode {:.0}x faster",
        encode_vs_swift, decode_vs_swift
    );
    if encode_vs_metal < 1.0 {
        println!(
            "  vs MetalBlurHash (GPU):  encode {:.1}x faster, decode {}",
            1.0 / encode_vs_metal,
            if decode_vs_metal < 1.0 {
                format!("{:.1}x faster", 1.0 / decode_vs_metal)
            } else {
                format!("{:.1}x slower (GPU wins)", decode_vs_metal)
            }
        );
    } else {
        println!(
            "  vs MetalBlurHash (GPU):  encode {:.1}x slower (GPU wins), decode {:.1}x {}",
            encode_vs_metal,
            if decode_vs_metal > 1.0 {
                decode_vs_metal
            } else {
                1.0 / decode_vs_metal
            },
            if decode_vs_metal > 1.0 {
                "slower (GPU wins)"
            } else {
                "faster"
            }
        );
    }

    println!("\nNote: MetalBlurHash uses GPU (Metal) acceleration.");
    println!("blurhash-rs is single-threaded CPU — no GPU required.");
    println!("blurhash-rs runs on any platform; MetalBlurHash requires Apple Metal.");
}
