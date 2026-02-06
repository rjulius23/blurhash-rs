# BlurHash-RS Architecture

## Overview

BlurHash-RS is a high-performance Rust port of the [blurhash-python](https://github.com/halcy/blurhash-python) library. The goal is a **drop-in replacement** that is API-compatible with the original Python package while delivering 100x+ performance improvement. The Rust core is exposed to Python via PyO3/maturin and to TypeScript/Node.js via napi-rs.

---

## Workspace Structure

```
blurhash-rs/
  Cargo.toml                     # Workspace root
  crates/
    blurhash-core/               # Pure Rust library (no FFI dependencies)
      Cargo.toml
      src/
        lib.rs                   # Public re-exports, top-level API
        base83.rs                # Base83 encoding/decoding
        color.rs                 # sRGB <-> linear color space conversions
        encode_impl.rs           # BlurHash encoding (image -> hash string)
        decode_impl.rs           # BlurHash decoding (hash string -> pixel buffer)
        error.rs                 # Error types (thiserror)
      benches/
        blurhash_bench.rs        # Criterion benchmarks
  bindings/
    python/                      # PyO3 + maturin binding
      Cargo.toml
      pyproject.toml
      src/
        lib.rs                   # #[pymodule] exposing Python API
    typescript/                  # napi-rs binding
      Cargo.toml
      package.json
      build.rs                   # napi-build setup
      src/
        lib.rs                   # #[napi] exports
  docs/
    architecture.md              # This file
  reference/
    blurhash_python_original.py  # Original Python source (read-only reference)
```

---

## Module Responsibilities

### `blurhash-core`

The pure Rust library. Zero `unsafe` blocks. No FFI or platform-specific code. All logic lives here; binding crates are thin wrappers.

| Module      | Responsibility |
|-------------|----------------|
| `lib.rs`    | Re-exports public API: `encode`, `decode`, `components`, `BlurhashError`. |
| `base83.rs` | Base83 alphabet, `encode(value, length) -> Result<String>`, `decode(str) -> Result<u64>`. Uses a compile-time `[u8; 128]` lookup table for O(1) character-to-value mapping. |
| `color.rs`  | `srgb_to_linear(u8) -> f64` (LUT-accelerated), `linear_to_srgb(f64) -> u8`, `sign_pow(f64, f64) -> f64`. All pure functions, no allocation. Includes a compile-time sRGB LUT built via Newton's method `const fn` for `pow`. |
| `encode_impl.rs` | `encode(pixels: &[u8], width, height, components_x, components_y) -> Result<String>`. Takes flat RGB byte buffer. Internally converts to linear, precomputes cosine tables, computes DCT, quantizes, and emits base83. |
| `decode_impl.rs` | `decode(blurhash, width, height, punch) -> Result<Vec<u8>>`. Returns flat RGB byte buffer. Also exposes `components(blurhash) -> Result<(u32, u32)>`. |
| `error.rs`  | `BlurhashError` enum covering all failure modes. |

### `bindings/python`

Thin PyO3 wrapper. Exposes:

- `encode(data: bytes, width: int, height: int, components_x: int = 4, components_y: int = 4) -> str`
- `decode(blurhash: str, width: int, height: int, punch: float = 1.0) -> bytes`
- `components(blurhash: str) -> tuple[int, int]`
- `srgb_to_linear(value: int) -> float`
- `linear_to_srgb(value: float) -> int`

### `bindings/typescript`

Thin napi-rs wrapper exposing:

- `encode(data: Buffer, width: number, height: number, componentsX?: number, componentsY?: number) -> string`
- `decode(blurhash: string, width: number, height: number, punch?: number) -> Buffer`
- `getComponents(blurhash: string) -> { componentsX: number, componentsY: number }`
- `srgbToLinear(value: number) -> number`
- `linearToSrgb(value: number) -> number`

---

## Public API Contracts

### Rust (`blurhash-core`)

```rust
/// Encode flat RGB pixel data into a BlurHash string.
///
/// `pixels` is a flat row-major RGB byte buffer of length `width * height * 3`.
/// Values are in sRGB space (0-255).
pub fn encode(
    pixels: &[u8],
    width: u32,
    height: u32,
    components_x: u32,
    components_y: u32,
) -> Result<String, BlurhashError>;

/// Decode a BlurHash string into flat RGB pixel data.
///
/// Returns a `Vec<u8>` of length `width * height * 3` containing sRGB pixel data
/// in row-major order.
pub fn decode(
    blurhash: &str,
    width: u32,
    height: u32,
    punch: f64,
) -> Result<Vec<u8>, BlurhashError>;

/// Extract the component counts from a BlurHash string.
pub fn components(blurhash: &str) -> Result<(u32, u32), BlurhashError>;
```

The flat `&[u8]` / `Vec<u8>` API was chosen over nested `Vec<Vec<...>>` for:
- Zero-copy FFI: the buffer can be passed directly to PyO3 `PyBytes` or napi `Buffer`.
- Cache locality: a single contiguous allocation with no pointer indirection.
- Simplicity: width/height are explicit parameters, not inferred from nested structure.

---

## Error Handling Strategy

All errors use a single `BlurhashError` enum defined with `thiserror`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BlurhashError {
    #[error("invalid BlurHash length: expected {expected}, got {actual}")]
    InvalidLength { expected: usize, actual: usize },

    #[error("component count out of range: {component} = {value} (must be 1..=9)")]
    InvalidComponentCount { component: &'static str, value: u32 },

    #[error("invalid base83 character: {0:?}")]
    InvalidBase83Character(char),

    #[error("encoding error: {0}")]
    EncodingError(String),
}
```

**Binding layers** convert `BlurhashError` into the native error type:
- Python: `PyErr` via `PyValueError::new_err(e.to_string())`
- TypeScript: napi `Error::from_reason(e.to_string())`

---

## Performance Optimization Strategy

### 1. Lookup Tables (LUT)

- **Base83 decode**: A `[u8; 128]` ASCII lookup table, built at compile time with `const fn`, replacing the Python dictionary lookup. O(1) per character.
- **sRGB-to-linear**: A `[f64; 256]` precomputed table, built at compile time using Newton's method for `const fn` power computation, eliminating `pow()` calls at runtime entirely.

### 2. Cache-Friendly Memory Layout

- Pixel data stored as flat `&[u8]` (input) and `Vec<u8>` (output) -- single contiguous allocation.
- Internal linear-RGB working buffer is `Vec<[f64; 3]>` -- flat, contiguous, no nested indirection.
- DCT basis values precomputed per-row and per-column into separate `Vec<Vec<f64>>` arrays, enabling the inner loop to be a simple multiply-accumulate without recomputing `cos()`.

### 3. DCT Basis Precomputation

The inner loop of both encode and decode computes `cos(pi * i * x / width) * cos(pi * j * y / height)`. Instead of computing this for every `(x, y, i, j)` combination:

1. Precompute `cos_x[i][x] = cos(pi * i * x / width)` for all `i` in `0..components_x` and `x` in `0..width`.
2. Precompute `cos_y[j][y] = cos(pi * j * y / height)` for all `j` in `0..components_y` and `y` in `0..height`.
3. The basis is then `cos_x[i][x] * cos_y[j][y]` -- two array lookups and one multiply instead of two transcendental function calls.

This reduces the number of `cos()` evaluations from `O(width * height * cx * cy)` to `O(width * cx + height * cy)`.

### 4. SIMD (Future Enhancement)

The multiply-accumulate loop over RGB channels is a natural candidate for SIMD. Strategy:

- Start with scalar code that auto-vectorizes well (using `[f64; 3]` with separate `r_sum/g_sum/b_sum` accumulators).
- Profile with `cargo bench` to establish baseline.
- If the compiler does not auto-vectorize, consider `std::simd` (nightly) or `packed_simd2` behind a feature flag.
- SIMD is a stretch goal; the LUT + precomputation approach should already achieve the 100x target.

### 5. Avoiding Allocation in Hot Paths

- Encode: single `String` buffer with `String::with_capacity` sized to the exact output length (`4 + 2 * cx * cy`).
- Decode: single `Vec::with_capacity` allocation for the pixel buffer (`width * height * 3`).
- Base83 encode: `vec![0u8; length]` stack-sized buffer, converted to `String` after filling.

---

## Coding Standards

1. **No `unsafe`** without written justification in a comment and approval from the architect. The core library must be 100% safe Rust.
2. **`#[deny(clippy::all)]` and `#[warn(clippy::pedantic)]`** at the crate level. All clippy warnings must be resolved or explicitly allowed with justification.
3. **`#![deny(missing_docs)]`**: all public items must have doc comments.
4. **Full test coverage**: every public function must have unit tests covering normal cases, edge cases, and error cases. Encode/decode round-trip tests are mandatory.
5. **Property-based compatibility**: tests must verify that `encode(image)` produces the exact same hash as the Python reference for a set of known test vectors.
6. **Benchmarks**: criterion benchmarks for `encode` and `decode` at various image sizes (4x4, 32x32, 128x128, 512x512) and component counts (1x1, 4x4, 9x9).
7. **Error handling**: no `.unwrap()` or `.expect()` in library code. All fallible operations return `Result<T, BlurhashError>`. (Exception: the base83 encode function uses `.unwrap()` on `String::from_utf8` with an ASCII-only buffer, which is provably infallible.)
8. **Edition 2021**, Rust stable toolchain.
