#!/usr/bin/env python3
"""
Benchmark the Rust-backed Python blurhash binding (built via maturin / PyO3).

Uses the same test cases as bench_python_original.py so results are directly
comparable.

Prerequisites:
    cd bindings/python
    maturin develop --release

Usage:
    python benchmarks/bench_python_binding.py
"""

import sys
import time

try:
    import blurhash as blurhash_rs
except ImportError:
    print(
        "ERROR: Could not import 'blurhash' (the Rust binding).\n"
        "Build it first with:  cd bindings/python && maturin develop --release",
        file=sys.stderr,
    )
    sys.exit(1)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def gradient_image_flat(width: int, height: int) -> list[int]:
    """Generate a gradient test image as a flat list of RGB u8 values."""
    pixels = []
    for y in range(height):
        for x in range(width):
            r = int((x / width) * 255)
            g = int((y / height) * 255)
            b = 128
            pixels.extend([r, g, b])
    return pixels


def gradient_image_nested(width: int, height: int) -> list:
    """Generate a gradient test image as nested lists [y][x][rgb]."""
    image = []
    for y in range(height):
        row = []
        for x in range(width):
            r = int((x / width) * 255)
            g = int((y / height) * 255)
            b = 128
            row.append([r, g, b])
        image.append(row)
    return image


def benchmark(label: str, func, iterations: int) -> float:
    """Run func() for the given number of iterations and print timing."""
    # Warm-up
    func()

    start = time.perf_counter()
    for _ in range(iterations):
        func()
    elapsed = time.perf_counter() - start

    per_iter_us = (elapsed / iterations) * 1_000_000
    per_iter_ms = (elapsed / iterations) * 1_000
    if per_iter_ms >= 1.0:
        print(f"  {label:40s}  {per_iter_ms:10.3f} ms/iter  ({iterations} iters)")
    else:
        print(f"  {label:40s}  {per_iter_us:10.1f} us/iter  ({iterations} iters)")
    return elapsed / iterations


# ---------------------------------------------------------------------------
# Benchmarks
# ---------------------------------------------------------------------------

def main():
    print("=" * 72)
    print("BlurHash Rust Python binding benchmark")
    print("=" * 72)
    print()

    # Detect the API style.  The binding may expose:
    #   encode(pixels, width, height, components_x, components_y) -> str
    #   decode(blurhash, width, height, punch) -> list[int]
    # or the Python-style nested-list API.  We try flat first.

    has_flat_api = hasattr(blurhash_rs, "encode")

    results = {}

    # ------------------------------------------------------------------
    # Encode benchmarks
    # ------------------------------------------------------------------
    print("--- Encode (4x3 components) ---")
    for size in [32, 128, 256, 512]:
        if has_flat_api:
            img = gradient_image_flat(size, size)
            encode_fn = lambda img=img, s=size: blurhash_rs.encode(img, s, s, 4, 3)
        else:
            img = gradient_image_nested(size, size)
            encode_fn = lambda img=img: blurhash_rs.blurhash_encode(img, 4, 3)
        iters = max(5, 1000 // (size * size // 1024 + 1))
        label = f"encode {size}x{size}"
        t = benchmark(label, encode_fn, iters)
        results[label] = t
    print()

    # ------------------------------------------------------------------
    # Encode with different component counts (128x128)
    # ------------------------------------------------------------------
    print("--- Encode component counts (128x128) ---")
    if has_flat_api:
        img128 = gradient_image_flat(128, 128)
    else:
        img128 = gradient_image_nested(128, 128)

    for cx, cy in [(1, 1), (4, 3), (4, 4), (9, 9)]:
        iters = 50 if (cx * cy) <= 16 else 10
        label = f"encode 128x128 {cx}x{cy}"
        if has_flat_api:
            fn = lambda cx=cx, cy=cy: blurhash_rs.encode(img128, 128, 128, cx, cy)
        else:
            fn = lambda cx=cx, cy=cy: blurhash_rs.blurhash_encode(img128, cx, cy)
        t = benchmark(label, fn, iters)
        results[label] = t
    print()

    # ------------------------------------------------------------------
    # Decode benchmarks
    # ------------------------------------------------------------------
    print("--- Decode (4x3 components) ---")
    if has_flat_api:
        img64 = gradient_image_flat(64, 64)
        hash_4x3 = blurhash_rs.encode(img64, 64, 64, 4, 3)
    else:
        img64 = gradient_image_nested(64, 64)
        hash_4x3 = blurhash_rs.blurhash_encode(img64, 4, 3)

    for size in [32, 128, 256]:
        iters = max(10, 2000 // (size * size // 1024 + 1))
        label = f"decode to {size}x{size}"
        if has_flat_api:
            fn = lambda s=size: blurhash_rs.decode(hash_4x3, s, s, 1.0)
        else:
            fn = lambda s=size: blurhash_rs.blurhash_decode(hash_4x3, s, s)
        t = benchmark(label, fn, iters)
        results[label] = t
    print()

    # ------------------------------------------------------------------
    # Summary
    # ------------------------------------------------------------------
    print("=" * 72)
    print("Summary (selected, ms/iter):")
    for label, t in results.items():
        print(f"  {label:40s}  {t * 1000:10.3f} ms")
    print("=" * 72)


if __name__ == "__main__":
    main()
