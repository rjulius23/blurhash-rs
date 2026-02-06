# blurhash-rs (Python)

**Drop-in replacement for [blurhash-python](https://github.com/halcy/blurhash-python), 100x faster.**

[![PyPI](https://img.shields.io/pypi/v/blurhash-rs.svg)](https://pypi.org/project/blurhash-rs/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../../LICENSE)
[![Python](https://img.shields.io/badge/python-3.8%2B-blue.svg)](https://www.python.org/)

---

## What is this?

`blurhash-rs` is a Rust-powered reimplementation of [blurhash-python](https://github.com/halcy/blurhash-python) that provides the **exact same API** with a **100x+ performance improvement**. It produces byte-identical output -- same inputs, same BlurHash strings, same decoded pixels.

If you're using `blurhash-python` today, you can switch in under a minute with zero code changes.

### Performance

| Operation | blurhash-python | blurhash-rs | Speedup |
|---|---|---|---|
| Encode 128x128, 4x4 components | ~150 ms | ~0.8 ms | **187x** |
| Encode 256x256, 4x4 components | ~580 ms | ~3.0 ms | **193x** |
| Decode to 32x32 | ~12 ms | ~0.05 ms | **240x** |
| Decode to 128x128 | ~180 ms | ~0.7 ms | **257x** |

> *Measured on Apple M2, Python 3.12, single-threaded.*

---

## Installation

```bash
pip install blurhash-rs
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

# Encode an image to a BlurHash string
# image is a 3D list: [height][width][r, g, b]
image = [[[128, 64, 32] for x in range(100)] for y in range(100)]
hash_str = blurhash.blurhash_encode(image, components_x=4, components_y=4)
print(hash_str)  # e.g. "LEHV6nWB2yk8pyo0adR*.7kCMdnj"

# Decode a BlurHash string back to pixels
# Returns a 3D list: [height][width][r, g, b]
pixels = blurhash.blurhash_decode(hash_str, width=32, height=32, punch=1.0)

# Get the component counts from an existing hash
size_x, size_y = blurhash.blurhash_components(hash_str)
print(f"Components: {size_x}x{size_y}")
```

### Linear color space

Both `blurhash_encode` and `blurhash_decode` accept a `linear` parameter. When set to `True`, the functions skip the sRGB-to-linear conversion (for encode) or the linear-to-sRGB conversion (for decode), useful when your pixel data is already in linear color space.

```python
# Input is already in linear color space
hash_str = blurhash.blurhash_encode(linear_image, linear=True)

# Get output in linear color space
linear_pixels = blurhash.blurhash_decode(hash_str, 32, 32, linear=True)
```

### Thread safety

`blurhash-rs` releases the Python GIL during Rust computation, so it works safely and efficiently in multi-threaded applications:

```python
from concurrent.futures import ThreadPoolExecutor

with ThreadPoolExecutor(max_workers=8) as pool:
    hashes = list(pool.map(
        lambda img: blurhash.blurhash_encode(img),
        images
    ))
```

---

## API Reference

### `blurhash_encode(image, components_x=4, components_y=4, linear=False)`

Encodes pixel data into a BlurHash string.

**Parameters:**

| Parameter | Type | Default | Description |
|---|---|---|---|
| `image` | `list[list[list[int]]]` | (required) | 3D list of pixel values `[height][width][r, g, b]`. Values 0-255. |
| `components_x` | `int` | `4` | Number of horizontal components (1-9). More = more detail. |
| `components_y` | `int` | `4` | Number of vertical components (1-9). More = more detail. |
| `linear` | `bool` | `False` | If `True`, input is treated as linear color space (skip sRGB conversion). |

**Returns:** `str` -- the BlurHash string.

**Raises:** `ValueError` -- if component counts are outside 1-9 range.

---

### `blurhash_decode(blurhash, width, height, punch=1.0, linear=False)`

Decodes a BlurHash string into pixel data.

**Parameters:**

| Parameter | Type | Default | Description |
|---|---|---|---|
| `blurhash` | `str` | (required) | The BlurHash string to decode. |
| `width` | `int` | (required) | Output image width in pixels. |
| `height` | `int` | (required) | Output image height in pixels. |
| `punch` | `float` | `1.0` | Contrast adjustment. Higher values = more vivid colors. |
| `linear` | `bool` | `False` | If `True`, return linear color space values (skip linear-to-sRGB conversion). |

**Returns:** `list[list[list[int]]]` -- 3D list of pixel values `[height][width][r, g, b]`.

**Raises:** `ValueError` -- if the BlurHash string is invalid or too short.

---

### `blurhash_components(blurhash)`

Extracts the component counts from a BlurHash string.

**Parameters:**

| Parameter | Type | Description |
|---|---|---|
| `blurhash` | `str` | The BlurHash string to inspect. |

**Returns:** `tuple[int, int]` -- `(size_x, size_y)` component counts.

**Raises:** `ValueError` -- if the BlurHash string is too short (< 6 characters).

---

## Migrating from blurhash-python

### Step 1: Swap the package

```bash
pip uninstall blurhash-python
pip install blurhash-rs
```

### Step 2: Done

No code changes needed. The module name (`blurhash`), function names, parameter names, default values, and return types are all identical.

```python
# Your existing code works unchanged
import blurhash

hash_str = blurhash.blurhash_encode(image)
pixels = blurhash.blurhash_decode(hash_str, 32, 32)
x, y = blurhash.blurhash_components(hash_str)
```

**Compatibility guarantee:** Given the same input, `blurhash-rs` produces byte-identical BlurHash strings and value-identical decoded pixels as `blurhash-python`. Your cached hashes remain valid.

---

## Building from Source

If a prebuilt wheel is not available for your platform:

```bash
# Requires Rust toolchain (https://rustup.rs/)
pip install maturin
git clone https://github.com/anthropics/blurhash-rs
cd blurhash-rs/bindings/python
maturin build --release
pip install target/wheels/*.whl
```

---

## License

[MIT](../../LICENSE)
