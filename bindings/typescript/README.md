# blurhash-rs (TypeScript / Node.js)

**Native BlurHash encoding and decoding for Node.js, powered by Rust.**

[![npm](https://img.shields.io/npm/v/blurhash-rs.svg)](https://www.npmjs.com/package/blurhash-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../../LICENSE)
[![Node.js](https://img.shields.io/badge/node-18%2B-green.svg)](https://nodejs.org/)

---

## What is this?

`blurhash-rs` is a high-performance [BlurHash](https://blurha.sh/) encoder and decoder for Node.js. It's written in Rust and compiled to a native N-API addon -- no WASM overhead, no JavaScript fallback, just raw native speed.

### Why not WASM?

| | Native (blurhash-rs) | WASM-based |
|---|---|---|
| Startup overhead | None (loaded by V8 natively) | WASM module compilation |
| Memory access | Direct, zero-copy buffers | Copies across WASM boundary |
| SIMD | Full CPU SIMD when available | Limited WASM SIMD support |
| Thread safety | Full N-API thread support | Single-threaded |

### Performance

| Operation | Pure JS | blurhash-rs (native) | Speedup |
|---|---|---|---|
| Encode 128x128, 4x4 components | ~45 ms | ~0.8 ms | **56x** |
| Encode 256x256, 4x4 components | ~170 ms | ~3.0 ms | **57x** |
| Decode to 32x32 | ~5 ms | ~0.05 ms | **100x** |
| Decode to 128x128 | ~70 ms | ~0.7 ms | **100x** |

> *Measured on Apple M2, Node.js 20, single-threaded.*

---

## Installation

```bash
npm install blurhash-rs
```

Prebuilt native binaries are available for:

| Platform | Architectures |
|---|---|
| Linux (glibc) | x86_64, aarch64 |
| macOS | x86_64, Apple Silicon (arm64) |
| Windows | x86_64 |

Node.js 18+ required. No Rust toolchain needed for installation.

---

## Usage

### TypeScript

```typescript
import { encode, decode, components } from 'blurhash-rs';

// Encode: flat Uint8Array of RGB pixel data (width * height * 3 bytes)
const pixels = new Uint8Array(width * height * 3);
// ... fill with RGB pixel data ...
const hash = encode(pixels, width, height, 4, 4);
console.log(`BlurHash: ${hash}`);

// Decode: returns Uint8Array of RGB pixel data
const decoded = decode(hash, 32, 32, 1.0);
// decoded.length === 32 * 32 * 3

// Get component counts from an existing hash
const { x, y } = components(hash);
console.log(`Components: ${x}x${y}`);
```

### JavaScript (CommonJS)

```javascript
const { encode, decode, components } = require('blurhash-rs');

const hash = encode(pixelBuffer, 128, 128);
const pixels = decode(hash, 32, 32);
const { x, y } = components(hash);
```

### With sharp (image processing)

```typescript
import sharp from 'sharp';
import { encode, decode } from 'blurhash-rs';

// Encode an image file to BlurHash
async function imageToBlurHash(path: string): Promise<string> {
  const { data, info } = await sharp(path)
    .raw()
    .ensureAlpha(false)
    .toBuffer({ resolveWithObject: true });

  // sharp outputs RGB buffer, exactly what blurhash-rs expects
  return encode(new Uint8Array(data), info.width, info.height, 4, 4);
}

// Decode a BlurHash to a PNG placeholder
async function blurHashToImage(hash: string, width: number, height: number): Promise<Buffer> {
  const pixels = decode(hash, width, height);
  return sharp(Buffer.from(pixels), {
    raw: { width, height, channels: 3 }
  }).png().toBuffer();
}
```

---

## API Reference

### `encode(pixels, width, height, componentX?, componentY?)`

Encodes RGB pixel data into a BlurHash string.

| Parameter | Type | Default | Description |
|---|---|---|---|
| `pixels` | `Uint8Array` | (required) | Flat array of RGB pixel data. Length must be `width * height * 3`. |
| `width` | `number` | (required) | Image width in pixels. |
| `height` | `number` | (required) | Image height in pixels. |
| `componentX` | `number` | `4` | Horizontal component count (1-9). |
| `componentY` | `number` | `4` | Vertical component count (1-9). |

**Returns:** `string` -- the BlurHash string.

**Throws:** `Error` -- if component counts are outside 1-9 or pixel buffer has wrong length.

---

### `decode(blurhash, width, height, punch?)`

Decodes a BlurHash string into RGB pixel data.

| Parameter | Type | Default | Description |
|---|---|---|---|
| `blurhash` | `string` | (required) | The BlurHash string to decode. |
| `width` | `number` | (required) | Output width in pixels. |
| `height` | `number` | (required) | Output height in pixels. |
| `punch` | `number` | `1.0` | Contrast factor. Higher = more vivid. |

**Returns:** `Uint8Array` -- flat RGB pixel data, length `width * height * 3`.

**Throws:** `Error` -- if the BlurHash string is invalid.

---

### `components(blurhash)`

Extracts component counts from a BlurHash string.

| Parameter | Type | Description |
|---|---|---|
| `blurhash` | `string` | The BlurHash string to inspect. |

**Returns:** `{ x: number, y: number }` -- the component counts.

**Throws:** `Error` -- if the BlurHash string is too short.

---

## TypeScript Types

Full type declarations are included with the package (`index.d.ts`):

```typescript
export function encode(
  pixels: Uint8Array,
  width: number,
  height: number,
  componentX?: number,
  componentY?: number,
): string;

export function decode(
  blurhash: string,
  width: number,
  height: number,
  punch?: number,
): Uint8Array;

export function components(blurhash: string): { x: number; y: number };
```

---

## Building from Source

If a prebuilt binary is not available for your platform:

```bash
# Requires Rust toolchain (https://rustup.rs/) and Node.js 18+
git clone https://github.com/anthropics/blurhash-rs
cd blurhash-rs/bindings/typescript
npm install
npm run build
```

---

## License

[MIT](../../LICENSE)
