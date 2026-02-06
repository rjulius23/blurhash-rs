# BlurHash-RS Test Plan

## 1. Correctness Test Strategy

### 1.1 Unit Tests (in `blurhash-core/src/`)

Each module should include inline `#[cfg(test)]` unit tests covering:

- **base83**: encode/decode roundtrip for boundary values (0, 82, 83, max values), invalid character rejection, length validation.
- **srgb_to_linear / linear_to_srgb**: roundtrip accuracy within 1 value for all 256 sRGB inputs, boundary values (0, 255), clamping behaviour for out-of-range linear values.
- **sign_pow**: positive, negative, and zero inputs with various exponents.

### 1.2 Integration Tests (`blurhash-core/tests/integration_tests.rs`)

Scope: end-to-end correctness through the public API (`encode`, `decode`, `components`, `base83`).

| Category | Test cases |
|---|---|
| **Known test vectors** | Decode `LEHV6nWB2yk8pyo0adR*.7kCMdnj` and verify output dimensions and pixel ranges |
| **DC-only** | A 1x1-component hash for a solid white image should decode to all-white pixels |
| **Encode -> Decode roundtrip** | Solid red/green/blue images should preserve dominant channel through encode-decode |
| **Component extraction** | `components()` should return the correct (x, y) counts from a hash string |
| **Hash length** | Encoded hash length must equal `4 + 2 * cx * cy` |
| **Character set** | All characters in encoded output must be valid base83 |
| **Determinism** | Encoding the same image twice must produce the same hash |
| **Error cases** | Invalid hash length, invalid characters, hash too short, component counts out of range (0, 10) |
| **Edge cases** | 1x1 image, 256x256 image, 1x1 components, 9x9 components, non-square images |
| **All component counts** | Iterate cx=1..9, cy=1..9 and verify encode-decode roundtrip |

### 1.3 Cross-Language Correctness

- Encode a set of canonical test images with the Python reference and compare hash output with Rust.
- Decode a set of canonical hashes with both Python and Rust and compare pixel output (allowing for rounding differences of +/- 1 per channel).

## 2. Performance Benchmark Methodology

### 2.1 Rust Criterion Benchmarks (`blurhash-core/benches/blurhash_bench.rs`)

| Benchmark group | Parameters |
|---|---|
| `encode` | Image sizes: 32x32, 128x128, 256x256, 512x512 (4x3 components) |
| `encode_components` | Component counts: 1x1, 4x3, 4x4, 9x9 (128x128 image) |
| `decode` | Output sizes: 32x32, 128x128, 256x256 (4x3 components) |
| `decode_components` | Component counts: 1x1, 4x3, 4x4, 9x9 (64x64 output) |
| `base83` | Encode 4 chars, encode 2 chars, decode 4 chars, decode full hash |
| `srgb_linear` | srgb_to_linear and linear_to_srgb over 256 values |

All benchmarks use `Throughput::Elements` where applicable so Criterion reports ops/sec. Each benchmark uses a synthetic gradient image for reproducibility.

### 2.2 Cross-Language Benchmark Suite (`benchmarks/`)

Three scripts with identical test cases for direct comparison:

| Script | Implementation |
|---|---|
| `bench_python_original.py` | Original Python blurhash (reference) |
| `bench_python_binding.py` | Rust-backed PyO3 binding |
| `bench_typescript.ts` | Rust-backed N-API binding |

Each script benchmarks:
- Encode at 32x32, 128x128, 256x256, 512x512
- Encode with different component counts at 128x128
- Decode to 32x32, 128x128, 256x256
- Base83 encode/decode (Python original only)
- sRGB/linear conversions (Python original only)

Output format: `label  time/iter  (iterations)` for easy side-by-side comparison.

### 2.3 Performance Target

The primary target is **100x+ speedup** over the Python reference implementation for both encode and decode at 128x128 with 4x3 components.

### 2.4 Measurement Best Practices

- Use `time.perf_counter()` in Python (not `time.time()`)
- Use `performance.now()` in TypeScript
- Use Criterion's statistical framework in Rust (automatic warm-up, outlier detection, confidence intervals)
- Run benchmarks on a quiet system with fixed CPU frequency when possible
- Always include warm-up iterations before measurement

## 3. Cross-Platform Testing Matrix

| Platform | Rust tests | Python binding | TypeScript binding |
|---|---|---|---|
| macOS (ARM64 / Apple Silicon) | `cargo test` | `maturin develop --release` | `napi build --release` |
| macOS (x86_64) | `cargo test` | `maturin develop --release` | `napi build --release` |
| Linux (x86_64, glibc) | `cargo test` | `maturin develop --release` | `napi build --release` |
| Linux (x86_64, musl) | `cargo test` | `maturin develop --release` | N/A |
| Windows (x86_64) | `cargo test` | `maturin develop --release` | `napi build --release` |

### CI Matrix (recommended)

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, macos-latest, windows-latest]
    rust: [stable, 1.70.0]  # MSRV
```

### Required tooling per platform

- Rust 1.70+ (MSRV from Cargo.toml)
- Python 3.8+ with maturin
- Node.js 18+ with @napi-rs/cli

## 4. Regression Detection Approach

### 4.1 Criterion Baselines

Criterion automatically stores baseline results in `target/criterion/`. To detect regressions:

```bash
# Save baseline after a known-good state
cargo bench -- --save-baseline main

# After changes, compare against the baseline
cargo bench -- --baseline main
```

Criterion will report percentage change and flag statistically significant regressions.

### 4.2 CI Integration

1. Run `cargo bench` on every PR and store the JSON output as a CI artifact.
2. Use `critcmp` to compare PR results against the main branch baseline.
3. Fail the CI check if any benchmark regresses by more than 10%.

```bash
# Install critcmp
cargo install critcmp

# Compare two baselines
critcmp main pr-branch
```

### 4.3 Cross-Language Regression

Run the Python and TypeScript benchmarks in CI and compare against stored baselines. A simple approach:

1. Store benchmark output as JSON (extend the scripts to add `--json` output).
2. Compare against previous runs using a threshold (e.g., 20% regression tolerance for binding overhead).

### 4.4 Correctness Regression

- `cargo test` must pass on every commit.
- The integration test suite covers all known edge cases and error conditions.
- Add new test cases for any bugs found during development or reported by users.
- Cross-language correctness tests should be run on release branches to ensure Python/TypeScript bindings produce identical results to the core library.
