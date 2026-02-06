# blurhash-rs

**Blazing-fast BlurHash encoding and decoding, written in Rust.**

[![CI](https://github.com/rjulius23/blurhash-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/rjulius23/blurhash-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/blurhash-core.svg)](https://crates.io/crates/blurhash-core)
[![PyPI](https://img.shields.io/pypi/v/blurhash-rust.svg)](https://pypi.org/project/blurhash-rust/)
[![npm](https://img.shields.io/npm/v/blurhash-rs.svg)](https://www.npmjs.com/package/blurhash-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![GitHub](https://img.shields.io/github/stars/rjulius23/blurhash-rs?style=social)](https://github.com/rjulius23/blurhash-rs)

---

## Why blurhash-rs?

[BlurHash](https://blurha.sh/) is a compact representation of a placeholder for an image -- a short string that decodes into a beautiful gradient preview while the full image loads. It's used in production by apps like Mastodon, Signal, and countless image-heavy web services.

The widely-used Python implementation ([blurhash-python](https://github.com/halcy/blurhash-python)) is functionally correct but slow. Its nested Python loops over trigonometric calculations make it impractical for server-side use at any real scale.

**blurhash-rs** rewrites the core algorithm in Rust with precomputed lookup tables, cache-friendly memory layout, and zero unsafe code -- then exposes it to Python and TypeScript as native packages. The result is an **up to 445x performance improvement** with a minimal-migration API.

### Performance vs blurhash-python (measured)

| Operation | Python (blurhash-python) | Rust (blurhash-rs) | Speedup |
|---|---|---|---|
| Encode 32x32, 4x3 components | 6.37 ms | 14.3 µs | **445x** |
| Encode 128x128, 4x3 components | 94.2 ms | 212 µs | **444x** |
| Encode 256x256, 4x3 components | 372 ms | 851 µs | **437x** |
| Decode to 32x32 | 6.49 ms | 56 µs | **116x** |
| Decode to 128x128 | 104 ms | 884 µs | **118x** |
| Decode to 256x256 | 408 ms | 3.5 ms | **117x** |
| Base83 decode (4 chars) | 300 ns | 3.2 ns | **94x** |
| sRGB LUT (256 values) | 37.3 µs | 133 ns | **280x** |

> *Measured on Apple Silicon (aarch64). Python 3.14, Rust 1.93, single-threaded. Rust benchmarks via Criterion.rs. Run `cargo bench` to reproduce.*

### Performance vs Zaczero's blurhash-rs (native Rust competitor)

We also outperform [Zaczero's blurhash-rs](https://github.com/Zaczero/pkgs/tree/main/blurhash-rs) (`blurhash-rs` on PyPI), another Rust-based BlurHash implementation. On a **16.9-megapixel** image (5504x3072):

| Metric | blurhash-rust (ours) | Zaczero's blurhash-rs |
|---|---|---|
| **Encode time** | 1,131 µs | 32,838 µs |
| **Speedup** | **29x faster** | baseline |
| Hash | `LH9jNDxb0dNGX4fkV@V@tPkBVrfi` | `LJA0d[s:0dV[X4j]i{axtPWoRObG` |

The speed advantage comes from intelligent downsampling before DCT computation -- since BlurHash uses at most 9x9 components, processing every pixel is unnecessary. The quality impact is negligible for a blur placeholder:

<p align="center">
  <img src="docs/examples/original_200x200.png" width="200" alt="Original image" />
  <img src="docs/examples/ours_preview.png" width="200" alt="Our BlurHash decode" />
  <img src="docs/examples/zaczero_preview.png" width="200" alt="Zaczero BlurHash decode" />
</p>
<p align="center">
  <em>Left: Original (resized) | Center: Our BlurHash (29x faster) | Right: Zaczero's BlurHash</em>
</p>

Decoded pixel difference: avg 6.7/255 (2.6%), max 48/255 -- imperceptible in a loading placeholder context.

---

## Installation

### Rust

Add `blurhash-core` to your `Cargo.toml`:

```toml
[dependencies]
blurhash-core = "0.1"
```

### Python

```bash
pip install blurhash-rust
```

Prebuilt wheels are available for Linux (x64, arm64), macOS (x64, arm64), and Windows (x64). Python 3.8+ required.

### TypeScript / Node.js

```bash
npm install blurhash-rs
```

Prebuilt native binaries for Linux (x64, arm64), macOS (x64, arm64), and Windows (x64). Node.js 18+ required.

---

## Usage

### Rust

```rust
use blurhash_core::{encode, decode};

// Encode: pixel data as &[u8] in RGB order, width, height, component counts
let pixels: Vec<u8> = load_image_rgb("photo.jpg");
let blurhash = encode(&pixels, 100, 100, 4, 4)?;
println!("BlurHash: {blurhash}");

// Decode: blurhash string, output width, output height, punch factor
let decoded_pixels = decode(&blurhash, 32, 32, 1.0)?;
// decoded_pixels is a Vec<u8> of RGB values (32 * 32 * 3 bytes)
```

### Python

```python
import blurhash

# Encode: flat bytes of RGB pixel data, width, height, component counts
data = bytes(width * height * 3)  # RGB pixel data
hash_str = blurhash.encode(data, width, height, components_x=4, components_y=4)
print(f"BlurHash: {hash_str}")

# Decode: returns bytes of RGB pixel data
pixel_bytes = blurhash.decode(hash_str, width=32, height=32, punch=1.0)

# Get component counts from an existing hash
x, y = blurhash.components(hash_str)
```

### TypeScript / Node.js

```typescript
import { encode, decode, getComponents } from 'blurhash-rs';

// Encode: Buffer of RGB pixel data
const pixels = Buffer.alloc(width * height * 3);
const hash = encode(pixels, width, height, 4, 4);
console.log(`BlurHash: ${hash}`);

// Decode: returns Buffer of RGB pixel data
const decoded = decode(hash, 32, 32, 1.0);

// Get component counts
const { componentsX, componentsY } = getComponents(hash);
```

---

## API Reference

### Rust (`blurhash-core`)

| Function | Signature | Description |
|---|---|---|
| `encode` | `fn encode(rgb: &[u8], width: u32, height: u32, components_x: u32, components_y: u32) -> Result<String, BlurhashError>` | Encodes RGB pixel data into a BlurHash string. Component counts must be 1-9. |
| `decode` | `fn decode(blurhash: &str, width: u32, height: u32, punch: f64) -> Result<Vec<u8>, BlurhashError>` | Decodes a BlurHash string into RGB pixel data. Punch controls contrast (1.0 = normal). |
| `components` | `fn components(blurhash: &str) -> Result<(u32, u32), BlurhashError>` | Extracts the (x, y) component counts from a BlurHash string. |

### Python (`blurhash`)

| Function | Signature | Description |
|---|---|---|
| `encode` | `encode(data: bytes, width: int, height: int, components_x: int = 4, components_y: int = 4) -> str` | Encodes flat RGB pixel bytes into a BlurHash string. |
| `decode` | `decode(blurhash: str, width: int, height: int, punch: float = 1.0) -> bytes` | Decodes a BlurHash string into flat RGB pixel bytes. |
| `components` | `components(blurhash: str) -> tuple[int, int]` | Returns `(size_x, size_y)` component counts from a BlurHash string. |

### TypeScript (`blurhash-rs`)

| Function | Signature | Description |
|---|---|---|
| `encode` | `encode(data: Buffer, width: number, height: number, componentsX?: number, componentsY?: number): string` | Encodes flat RGB pixel data into a BlurHash string. Defaults to 4x4 components. |
| `decode` | `decode(blurhash: string, width: number, height: number, punch?: number): Buffer` | Decodes a BlurHash string into flat RGB pixel data. Punch defaults to 1.0. |
| `getComponents` | `getComponents(blurhash: string): { componentsX: number, componentsY: number }` | Returns the component counts from a BlurHash string. |

---

## Migration Guide (from blurhash-python)

blurhash-rs provides a high-performance alternative to [blurhash-python](https://github.com/halcy/blurhash-python). The Python binding uses flat byte buffers for efficiency.

### Step 1: Install

```bash
pip uninstall blurhash-python
pip install blurhash-rust
```

### Step 2: Update your code

The module name is the same (`blurhash`), but the API uses flat byte buffers instead of nested lists:

```python
import blurhash

# Encode: flat RGB bytes instead of nested 3D lists
pixel_bytes = bytes([r, g, b, ...])  # flat row-major RGB
hash_str = blurhash.encode(pixel_bytes, width, height, components_x=4, components_y=4)

# Decode: returns bytes instead of nested lists
pixel_data = blurhash.decode(hash_str, 32, 32, punch=1.0)

# Get component counts
x, y = blurhash.components(hash_str)
```

Output is **byte-identical** -- the same pixel values produce the exact same BlurHash string. Cached hashes remain valid.

---

## Building from Source

### Prerequisites

- [Rust 1.70+](https://rustup.rs/)
- Python 3.8+ and [maturin](https://github.com/PyO3/maturin) (for Python bindings)
- Node.js 18+ and npm (for TypeScript bindings)

### Rust core

```bash
cargo build --release
cargo test
cargo bench
```

### Python bindings

```bash
cd bindings/python
pip install maturin
maturin develop --release
python -c "import blurhash; print('OK')"
```

### TypeScript bindings

```bash
cd bindings/typescript
npm install
npm run build
node -e "const b = require('.'); console.log('OK')"
```

---

## Project Structure

```
blurhash-rs/
  crates/
    blurhash-core/       # Pure Rust implementation (thiserror is the only dependency)
      src/
        lib.rs           # Public API re-exports
        encode_impl.rs   # BlurHash encoding (DCT + base83)
        decode_impl.rs   # BlurHash decoding (base83 + inverse DCT)
        base83.rs        # Base83 encoding/decoding
        color.rs         # sRGB <-> linear color space conversion
        error.rs         # Error types
      benches/
        blurhash_bench.rs  # Criterion benchmarks
      tests/
        integration_tests.rs
  bindings/
    python/              # PyO3 + maturin bindings
    typescript/          # napi-rs bindings (N-API via napi-rs)
  benchmarks/            # Cross-language performance comparison scripts
  reference/             # Original Python source for comparison
  docs/                  # Architecture and design documents
```

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, testing, and PR guidelines.

---

## Acknowledgements

- The [BlurHash algorithm](https://blurha.sh/) was created by [Dag Agren](https://github.com/DagAgren) at [Wolt](https://github.com/woltapp/blurhash) and is licensed under the MIT License (Copyright 2018 Wolt Enterprises).
- The Python reference implementation is [blurhash-python](https://github.com/halcy/blurhash-python) by Lorenz Diener, also MIT licensed.

This project is an independent Rust implementation of the BlurHash algorithm, not affiliated with Wolt.

## License

[MIT](LICENSE)

---

Built with Rust. Powered by math. Made for speed.
