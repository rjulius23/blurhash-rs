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
import { encode, decode, getComponents } from 'blurhash-rs';

// Encode: Buffer of RGB pixel data (width * height * 3 bytes)
const pixels = Buffer.alloc(width * height * 3);
// ... fill with RGB pixel data ...
const hash = encode(pixels, width, height, 4, 4);
console.log(`BlurHash: ${hash}`);

// Decode: returns Buffer of RGB pixel data
const decoded = decode(hash, 32, 32, 1.0);
// decoded.length === 32 * 32 * 3

// Get component counts from an existing hash
const { componentsX, componentsY } = getComponents(hash);
console.log(`Components: ${componentsX}x${componentsY}`);
```

### JavaScript (CommonJS)

```javascript
const { encode, decode, getComponents } = require('blurhash-rs');

const hash = encode(pixelBuffer, 128, 128);
const pixels = decode(hash, 32, 32);
const { componentsX, componentsY } = getComponents(hash);
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
  return encode(Buffer.from(data), info.width, info.height, 4, 4);
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

### `encode(data, width, height, componentsX?, componentsY?)`

Encodes RGB pixel data into a BlurHash string.

| Parameter | Type | Default | Description |
|---|---|---|---|
| `data` | `Buffer` | (required) | Raw RGB pixel data. Length must be `width * height * 3`. |
| `width` | `number` | (required) | Image width in pixels. |
| `height` | `number` | (required) | Image height in pixels. |
| `componentsX` | `number` | `4` | Horizontal component count (1-9). |
| `componentsY` | `number` | `4` | Vertical component count (1-9). |

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

**Returns:** `Buffer` -- flat RGB pixel data, length `width * height * 3`.

**Throws:** `Error` -- if the BlurHash string is invalid.

---

### `getComponents(blurhash)`

Extracts component counts from a BlurHash string.

| Parameter | Type | Description |
|---|---|---|
| `blurhash` | `string` | The BlurHash string to inspect. |

**Returns:** `{ componentsX: number, componentsY: number }` -- the component counts.

**Throws:** `Error` -- if the BlurHash string is too short.

---

### `srgbToLinear(value)`

Converts an sRGB byte value to linear RGB.

| Parameter | Type | Description |
|---|---|---|
| `value` | `number` | sRGB value in the range 0-255. |

**Returns:** `number` -- linear RGB value in the range 0.0-1.0.

---

### `linearToSrgb(value)`

Converts a linear RGB value to an sRGB byte value.

| Parameter | Type | Description |
|---|---|---|
| `value` | `number` | Linear RGB value in the range 0.0-1.0. |

**Returns:** `number` -- sRGB byte value in the range 0-255.

---

## TypeScript Types

Full type declarations are included with the package (`index.d.ts`):

```typescript
export function encode(
  data: Buffer,
  width: number,
  height: number,
  componentsX?: number,
  componentsY?: number,
): string;

export function decode(
  blurhash: string,
  width: number,
  height: number,
  punch?: number,
): Buffer;

export interface Components {
  componentsX: number;
  componentsY: number;
}

export function getComponents(blurhash: string): Components;

export function srgbToLinear(value: number): number;
export function linearToSrgb(value: number): number;
```

---

## Building from Source

If a prebuilt binary is not available for your platform:

```bash
# Requires Rust toolchain (https://rustup.rs/) and Node.js 18+
git clone https://github.com/rjulius23/blurhash-rs
cd blurhash-rs/bindings/typescript
npm install
npm run build
```

---

## License

[MIT](../../LICENSE)
