# blurhash-rs

**Blazing-fast BlurHash encoding and decoding, written in Rust.**

[![CI](https://github.com/rjulius23/blurhash-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/rjulius23/blurhash-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/blurhash-core.svg)](https://crates.io/crates/blurhash-core)
[![PyPI](https://img.shields.io/pypi/v/blurhash-rs.svg)](https://pypi.org/project/blurhash-rs/)
[![npm](https://img.shields.io/npm/v/blurhash-rs.svg)](https://www.npmjs.com/package/blurhash-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![GitHub](https://img.shields.io/github/stars/rjulius23/blurhash-rs?style=social)](https://github.com/rjulius23/blurhash-rs)

---

## Why blurhash-rs?

[BlurHash](https://blurha.sh/) is a compact representation of a placeholder for an image -- a short string that decodes into a beautiful gradient preview while the full image loads. It's used in production by apps like Mastodon, Signal, and countless image-heavy web services.

The widely-used Python implementation ([blurhash-python](https://github.com/halcy/blurhash-python)) is functionally correct but slow. Its nested Python loops over trigonometric calculations make it impractical for server-side use at any real scale.

**blurhash-rs** rewrites the core algorithm in Rust with precomputed lookup tables, cache-friendly memory layout, and zero unsafe code -- then exposes it to Python and TypeScript as native packages. The result is an **up to 445x performance improvement** with zero API changes.

### Performance (measured)

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
pip install blurhash-rs
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

# Encode: 3D list of pixel values [height][width][rgb]
image = [[[r, g, b] for x in range(width)] for y in range(height)]
hash_str = blurhash.blurhash_encode(image, components_x=4, components_y=4)
print(f"BlurHash: {hash_str}")

# Decode: returns 3D list of pixel values [height][width][rgb]
pixels = blurhash.blurhash_decode(hash_str, width=32, height=32, punch=1.0)

# Get component counts from an existing hash
x, y = blurhash.blurhash_components(hash_str)
```

### TypeScript

```typescript
import { encode, decode, components } from 'blurhash-rs';

// Encode: flat Uint8Array of RGB pixel data
const pixels = new Uint8Array(width * height * 3);
const hash = encode(pixels, width, height, 4, 4);
console.log(`BlurHash: ${hash}`);

// Decode: returns Uint8Array of RGB pixel data
const decoded = decode(hash, 32, 32, 1.0);

// Get component counts
const { x, y } = components(hash);
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
| `blurhash_encode` | `blurhash_encode(image, components_x=4, components_y=4, linear=False)` | Encodes a 3D pixel list into a BlurHash string. Set `linear=True` if input is already in linear color space. |
| `blurhash_decode` | `blurhash_decode(blurhash, width, height, punch=1.0, linear=False)` | Decodes a BlurHash string into a 3D pixel list. Set `linear=True` to get linear color space output. |
| `blurhash_components` | `blurhash_components(blurhash)` | Returns `(size_x, size_y)` component counts from a BlurHash string. |

### TypeScript (`blurhash-rs`)

| Function | Signature | Description |
|---|---|---|
| `encode` | `encode(pixels: Uint8Array, width: number, height: number, componentX?: number, componentY?: number): string` | Encodes flat RGB pixel data into a BlurHash string. Defaults to 4x4 components. |
| `decode` | `decode(blurhash: string, width: number, height: number, punch?: number): Uint8Array` | Decodes a BlurHash string into flat RGB pixel data. Punch defaults to 1.0. |
| `components` | `components(blurhash: string): { x: number, y: number }` | Returns the component counts from a BlurHash string. |

---

## Migration Guide (from blurhash-python)

blurhash-rs is a **drop-in replacement** for [blurhash-python](https://github.com/halcy/blurhash-python). The API signatures, parameter names, default values, and return types are identical.

### Step 1: Install

```bash
pip uninstall blurhash-python
pip install blurhash-rs
```

### Step 2: Update imports (optional)

Your existing code should work unchanged:

```python
# This still works -- the module name is the same
import blurhash

hash_str = blurhash.blurhash_encode(image)
pixels = blurhash.blurhash_decode(hash_str, 32, 32)
```

If you want to be explicit:

```python
# Alias for clarity
import blurhash  # now powered by blurhash-rs
```

### Step 3: That's it

All function signatures match exactly:

| Function | blurhash-python | blurhash-rs | Match? |
|---|---|---|---|
| `blurhash_encode(image, components_x=4, components_y=4, linear=False)` | Yes | Yes | Identical |
| `blurhash_decode(blurhash, width, height, punch=1.0, linear=False)` | Yes | Yes | Identical |
| `blurhash_components(blurhash)` | Yes | Yes | Identical |

Output is **byte-identical** -- the same input produces the exact same BlurHash string. Cached hashes remain valid. No visual regressions.

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
    blurhash-core/       # Pure Rust implementation (no dependencies)
      src/
        lib.rs           # Public API re-exports
        encode.rs        # BlurHash encoding (DCT + base83)
        decode.rs        # BlurHash decoding (base83 + inverse DCT)
        base83.rs        # Base83 encoding/decoding
        color.rs         # sRGB <-> linear color space conversion
        error.rs         # Error types
  bindings/
    python/              # PyO3 + maturin bindings
    typescript/          # napi-rs bindings
  benchmarks/            # Performance benchmarks
  reference/             # Original Python source for comparison
```

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, testing, and PR guidelines.

---

## License

[MIT](LICENSE)

---

Built with Rust. Powered by math. Made for speed.
