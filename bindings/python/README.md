# blurhash-rust (Python)

**High-performance BlurHash encoding and decoding, powered by Rust.**

[![PyPI](https://img.shields.io/pypi/v/blurhash-rust.svg)](https://pypi.org/project/blurhash-rust/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../../LICENSE)
[![Python](https://img.shields.io/badge/python-3.8%2B-blue.svg)](https://www.python.org/)

---

## What is this?

`blurhash-rust` is a Rust-powered Python package for [BlurHash](https://blurha.sh/) encoding and decoding. It provides a simple, fast API for generating compact BlurHash strings from raw pixel data and decoding them back to pixels.

---

## Installation

```bash
pip install blurhash-rust
```

Prebuilt wheels are available for:

| Platform | Architectures |
|---|---|
| Linux | x86_64, aarch64 |
| macOS | x86_64, Apple Silicon (arm64) |
| Windows | x86_64 |

Python 3.8+ required. No Rust toolchain needed for installation.

---

## Usage

```python
import blurhash

# Encode raw RGB pixel data into a BlurHash string
# `data` must be a bytes object of length width * height * 3 (RGB)
width, height = 100, 100
pixel_data = bytes([128, 64, 32] * (width * height))

hash_str = blurhash.encode(pixel_data, width, height, components_x=4, components_y=4)
print(hash_str)  # e.g. "LEHV6nWB2yk8pyo0adR*.7kCMdnj"

# Decode a BlurHash string back to raw RGB pixel data
# Returns a bytes object of length width * height * 3
raw_pixels = blurhash.decode(hash_str, width=32, height=32, punch=1.0)
print(len(raw_pixels))  # 32 * 32 * 3 = 3072

# Get the component counts from an existing hash
components_x, components_y = blurhash.components(hash_str)
print(f"Components: {components_x}x{components_y}")
```

### Working with images (Pillow)

```python
from PIL import Image
import blurhash

# Encode from a PIL Image
img = Image.open("photo.jpg").convert("RGB")
pixel_data = img.tobytes()
hash_str = blurhash.encode(pixel_data, img.width, img.height)

# Decode back to a PIL Image
raw = blurhash.decode(hash_str, width=32, height=32)
preview = Image.frombytes("RGB", (32, 32), raw)
preview.save("preview.png")
```

### Color space utilities

```python
import blurhash

# Convert between sRGB and linear color space
linear = blurhash.srgb_to_linear(128)   # 0.2158...
srgb = blurhash.linear_to_srgb(0.5)     # 188
```

---

## API Reference

### `encode(data, width, height, components_x=4, components_y=4)`

Encodes raw RGB pixel data into a BlurHash string.

**Parameters:**

| Parameter | Type | Default | Description |
|---|---|---|---|
| `data` | `bytes` | (required) | Raw pixel bytes in RGB order. Length must be `width * height * 3`. |
| `width` | `int` | (required) | Image width in pixels. |
| `height` | `int` | (required) | Image height in pixels. |
| `components_x` | `int` | `4` | Number of horizontal components (1-9). More = more detail. |
| `components_y` | `int` | `4` | Number of vertical components (1-9). More = more detail. |

**Returns:** `str` -- the BlurHash string.

**Raises:** `ValueError` -- if component counts are outside 1-9 range or data length does not match dimensions.

---

### `decode(blurhash, width, height, punch=1.0)`

Decodes a BlurHash string into raw RGB pixel data.

**Parameters:**

| Parameter | Type | Default | Description |
|---|---|---|---|
| `blurhash` | `str` | (required) | The BlurHash string to decode. |
| `width` | `int` | (required) | Output image width in pixels. |
| `height` | `int` | (required) | Output image height in pixels. |
| `punch` | `float` | `1.0` | Contrast adjustment. Higher values = more vivid colors. |

**Returns:** `bytes` -- raw RGB pixel data of length `width * height * 3`.

**Raises:** `ValueError` -- if the BlurHash string is invalid or too short.

---

### `components(blurhash)`

Extracts the component counts from a BlurHash string.

**Parameters:**

| Parameter | Type | Description |
|---|---|---|
| `blurhash` | `str` | The BlurHash string to inspect. |

**Returns:** `tuple[int, int]` -- `(components_x, components_y)` component counts.

**Raises:** `ValueError` -- if the BlurHash string is too short.

---

### `srgb_to_linear(value)`

Converts an sRGB byte value to linear RGB.

**Parameters:**

| Parameter | Type | Description |
|---|---|---|
| `value` | `int` | sRGB value (0-255). |

**Returns:** `float` -- linear RGB value (0.0-1.0).

---

### `linear_to_srgb(value)`

Converts a linear RGB value to an sRGB byte.

**Parameters:**

| Parameter | Type | Description |
|---|---|---|
| `value` | `float` | Linear RGB value (0.0-1.0). |

**Returns:** `int` -- sRGB value (0-255).

---

## Differences from blurhash-python

This package is **not** a drop-in replacement for [blurhash-python](https://github.com/halcy/blurhash-python). The API differs in several ways:

| | blurhash-python | blurhash-rust |
|---|---|---|
| **Encode function** | `blurhash_encode(image)` | `encode(data, width, height)` |
| **Decode function** | `blurhash_decode(hash, w, h)` | `decode(hash, w, h)` |
| **Components function** | `blurhash_components(hash)` | `components(hash)` |
| **Encode input** | 3D list `[h][w][r,g,b]` | flat `bytes` (RGB) |
| **Decode output** | 3D list `[h][w][r,g,b]` | flat `bytes` (RGB) |
| **Linear mode** | `linear=True` parameter | Use `srgb_to_linear()` / `linear_to_srgb()` utilities |

### Migrating from blurhash-python

```python
# Before (blurhash-python):
import blurhash
hash_str = blurhash.blurhash_encode(image_3d_list, components_x=4, components_y=4)
pixels_3d = blurhash.blurhash_decode(hash_str, 32, 32)
x, y = blurhash.blurhash_components(hash_str)

# After (blurhash-rust):
import blurhash
# Convert 3D list to flat bytes for encode
flat = bytes(c for row in image_3d_list for pixel in row for c in pixel)
hash_str = blurhash.encode(flat, width, height, components_x=4, components_y=4)
# decode returns flat bytes instead of 3D list
raw = blurhash.decode(hash_str, 32, 32)
x, y = blurhash.components(hash_str)
```

---

## Building from Source

If a prebuilt wheel is not available for your platform:

```bash
# Requires Rust toolchain (https://rustup.rs/)
pip install maturin
git clone https://github.com/rjulius23/blurhash-rs
cd blurhash-rs/bindings/python
maturin build --release
pip install target/wheels/*.whl
```

---

## License

[MIT](../../LICENSE)
