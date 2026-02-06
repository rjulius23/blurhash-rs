/**
 * Benchmark the Rust-backed TypeScript/N-API blurhash binding.
 *
 * Uses the same test cases as the Python benchmarks so results are directly
 * comparable.
 *
 * Prerequisites:
 *   cd bindings/typescript
 *   npm install
 *   npm run build          # or: napi build --release
 *
 * Usage:
 *   npx ts-node benchmarks/bench_typescript.ts
 *   # or
 *   npx tsx benchmarks/bench_typescript.ts
 */

// Try to load the native binding from the build output
let blurhash: {
  encode: (pixels: number[] | Uint8Array, width: number, height: number, componentsX: number, componentsY: number) => string;
  decode: (hash: string, width: number, height: number, punch?: number) => Uint8Array | number[];
};

try {
  blurhash = require("../bindings/typescript");
} catch {
  try {
    blurhash = require("../bindings/typescript/blurhash-native");
  } catch {
    console.error(
      "ERROR: Could not load the native blurhash binding.\n" +
        "Build it first with:  cd bindings/typescript && npm run build"
    );
    process.exit(1);
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function gradientImage(width: number, height: number): Uint8Array {
  const pixels = new Uint8Array(width * height * 3);
  let idx = 0;
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      pixels[idx++] = Math.floor((x / width) * 255);   // R
      pixels[idx++] = Math.floor((y / height) * 255);  // G
      pixels[idx++] = 128;                               // B
    }
  }
  return pixels;
}

interface BenchResult {
  label: string;
  perIterMs: number;
  iterations: number;
}

function benchmark(label: string, fn: () => void, iterations: number): BenchResult {
  // Warm-up
  fn();

  const start = performance.now();
  for (let i = 0; i < iterations; i++) {
    fn();
  }
  const elapsed = performance.now() - start;
  const perIterMs = elapsed / iterations;

  if (perIterMs >= 1.0) {
    console.log(
      `  ${label.padEnd(40)}  ${perIterMs.toFixed(3).padStart(10)} ms/iter  (${iterations} iters)`
    );
  } else {
    const perIterUs = perIterMs * 1000;
    console.log(
      `  ${label.padEnd(40)}  ${perIterUs.toFixed(1).padStart(10)} us/iter  (${iterations} iters)`
    );
  }

  return { label, perIterMs, iterations };
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

function main() {
  console.log("=".repeat(72));
  console.log("BlurHash Rust TypeScript/N-API binding benchmark");
  console.log("=".repeat(72));
  console.log();

  const results: BenchResult[] = [];

  // ------------------------------------------------------------------
  // Encode benchmarks
  // ------------------------------------------------------------------
  console.log("--- Encode (4x3 components) ---");
  for (const size of [32, 128, 256, 512]) {
    const img = gradientImage(size, size);
    const iters = Math.max(5, Math.floor(1000 / (((size * size) / 1024) + 1)));
    const r = benchmark(
      `encode ${size}x${size}`,
      () => blurhash.encode(img, size, size, 4, 3),
      iters
    );
    results.push(r);
  }
  console.log();

  // ------------------------------------------------------------------
  // Encode with different component counts (128x128)
  // ------------------------------------------------------------------
  console.log("--- Encode component counts (128x128) ---");
  const img128 = gradientImage(128, 128);
  for (const [cx, cy] of [[1, 1], [4, 3], [4, 4], [9, 9]]) {
    const iters = cx * cy <= 16 ? 50 : 10;
    const r = benchmark(
      `encode 128x128 ${cx}x${cy}`,
      () => blurhash.encode(img128, 128, 128, cx, cy),
      iters
    );
    results.push(r);
  }
  console.log();

  // ------------------------------------------------------------------
  // Decode benchmarks
  // ------------------------------------------------------------------
  console.log("--- Decode (4x3 components) ---");
  const img64 = gradientImage(64, 64);
  const hash4x3 = blurhash.encode(img64, 64, 64, 4, 3);

  for (const size of [32, 128, 256]) {
    const iters = Math.max(10, Math.floor(2000 / (((size * size) / 1024) + 1)));
    const r = benchmark(
      `decode to ${size}x${size}`,
      () => blurhash.decode(hash4x3, size, size, 1.0),
      iters
    );
    results.push(r);
  }
  console.log();

  // ------------------------------------------------------------------
  // Summary
  // ------------------------------------------------------------------
  console.log("=".repeat(72));
  console.log("Summary (selected, ms/iter):");
  for (const r of results) {
    console.log(`  ${r.label.padEnd(40)}  ${r.perIterMs.toFixed(3).padStart(10)} ms`);
  }
  console.log("=".repeat(72));
}

main();
