use blurhash_core::{base83, decode, encode};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// ---------------------------------------------------------------------------
// Helpers
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

// ---------------------------------------------------------------------------
// Encode benchmarks
// ---------------------------------------------------------------------------

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode");

    for &(w, h) in &[(32u32, 32u32), (128, 128), (256, 256), (512, 512)] {
        let img = gradient_image(w as usize, h as usize);
        let label = format!("{w}x{h}");
        group.throughput(Throughput::Elements((w as u64) * (h as u64)));
        group.bench_with_input(BenchmarkId::new("4x3", &label), &img, |b, img| {
            b.iter(|| encode(img, w, h, 4, 3).unwrap());
        });
    }

    group.finish();
}

fn bench_encode_component_counts(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_components");

    let img = gradient_image(128, 128);
    for &(cx, cy) in &[(1u32, 1u32), (4, 3), (4, 4), (9, 9)] {
        let label = format!("{cx}x{cy}");
        group.bench_with_input(BenchmarkId::new("128x128", &label), &img, |b, img| {
            b.iter(|| encode(img, 128, 128, cx, cy).unwrap());
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Decode benchmarks
// ---------------------------------------------------------------------------

fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode");

    // Pre-encode a hash to decode
    let img = gradient_image(64, 64);
    let hash = encode(&img, 64, 64, 4, 3).expect("encode ok");

    for &(w, h) in &[(32u32, 32u32), (128, 128), (256, 256)] {
        let label = format!("{w}x{h}");
        group.throughput(Throughput::Elements((w as u64) * (h as u64)));
        group.bench_with_input(BenchmarkId::new("4x3", &label), &hash, |b, hash| {
            b.iter(|| decode(hash, w, h, 1.0).unwrap());
        });
    }

    group.finish();
}

fn bench_decode_component_counts(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_components");

    let img = gradient_image(64, 64);
    for &(cx, cy) in &[(1u32, 1u32), (4, 3), (4, 4), (9, 9)] {
        let hash = encode(&img, 64, 64, cx, cy).expect("encode ok");
        let label = format!("{cx}x{cy}");
        group.bench_with_input(BenchmarkId::new("64x64", &label), &hash, |b, hash| {
            b.iter(|| decode(hash, 64, 64, 1.0).unwrap());
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Base83 benchmarks
// ---------------------------------------------------------------------------

fn bench_base83(c: &mut Criterion) {
    let mut group = c.benchmark_group("base83");

    group.bench_function("encode_4_chars", |b| {
        b.iter(|| base83::encode(123456, 4).unwrap());
    });

    group.bench_function("encode_2_chars", |b| {
        b.iter(|| base83::encode(1234, 2).unwrap());
    });

    group.bench_function("decode_4_chars", |b| {
        let s = base83::encode(123456, 4).unwrap();
        b.iter(|| base83::decode(&s).unwrap());
    });

    group.bench_function("decode_long", |b| {
        // Decode a full blurhash-length string (28 chars)
        let img = gradient_image(16, 16);
        let hash = encode(&img, 16, 16, 4, 3).expect("encode ok");
        b.iter(|| base83::decode(&hash).unwrap());
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// sRGB / linear conversion benchmarks
// ---------------------------------------------------------------------------

fn bench_srgb_linear(c: &mut Criterion) {
    let mut group = c.benchmark_group("srgb_linear");

    group.bench_function("srgb_to_linear_256_values", |b| {
        b.iter(|| {
            let mut sum = 0.0f64;
            for i in 0..=255u8 {
                sum += blurhash_core::srgb_to_linear(i);
            }
            sum
        });
    });

    group.bench_function("linear_to_srgb_256_values", |b| {
        b.iter(|| {
            let mut sum = 0u32;
            for i in 0..256u32 {
                let linear = i as f64 / 255.0;
                sum += blurhash_core::linear_to_srgb(linear) as u32;
            }
            sum
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion harness
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_encode,
    bench_encode_component_counts,
    bench_decode,
    bench_decode_component_counts,
    bench_base83,
    bench_srgb_linear,
);
criterion_main!(benches);
