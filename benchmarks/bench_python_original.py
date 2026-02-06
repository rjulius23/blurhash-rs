#!/usr/bin/env python3
"""
Benchmark the original Python blurhash implementation.

Generates synthetic gradient images and measures encode/decode times at
multiple image sizes so the results can be directly compared to the Rust
Criterion benchmarks and the Rust-backed Python binding benchmarks.

Usage:
    python benchmarks/bench_python_original.py
"""

import math
import sys
import time
import os

# Add the reference directory so we can import the original module
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "reference"))
import blurhash_python_original as blurhash

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def gradient_image(width: int, height: int) -> list:
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
    print("BlurHash Python (original) benchmark")
    print("=" * 72)
    print()

    results = {}

    # ------------------------------------------------------------------
    # Encode benchmarks
    # ------------------------------------------------------------------
    print("--- Encode (4x3 components) ---")
    for size in [32, 128, 256]:
        img = gradient_image(size, size)
        iters = {32: 20, 128: 5, 256: 2}[size]
        label = f"encode {size}x{size}"
        t = benchmark(label, lambda img=img: blurhash.blurhash_encode(img, 4, 3), iters)
        results[label] = t
    print()

    # ------------------------------------------------------------------
    # Encode with different component counts (128x128)
    # ------------------------------------------------------------------
    print("--- Encode component counts (128x128) ---")
    img128 = gradient_image(128, 128)
    for cx, cy in [(1, 1), (4, 3), (4, 4), (9, 9)]:
        iters = 5 if (cx * cy) <= 16 else 2
        label = f"encode 128x128 {cx}x{cy}"
        t = benchmark(label, lambda cx=cx, cy=cy: blurhash.blurhash_encode(img128, cx, cy), iters)
        results[label] = t
    print()

    # ------------------------------------------------------------------
    # Decode benchmarks
    # ------------------------------------------------------------------
    print("--- Decode (4x3 components) ---")
    img64 = gradient_image(64, 64)
    hash_4x3 = blurhash.blurhash_encode(img64, 4, 3)
    for size in [32, 128, 256]:
        iters = {32: 20, 128: 5, 256: 2}[size]
        label = f"decode to {size}x{size}"
        t = benchmark(label, lambda s=size: blurhash.blurhash_decode(hash_4x3, s, s), iters)
        results[label] = t
    print()

    # ------------------------------------------------------------------
    # Base83 benchmarks
    # ------------------------------------------------------------------
    print("--- Base83 ---")
    benchmark("base83 encode (4 chars)", lambda: blurhash.base83_encode(123456, 4), 10000)
    benchmark("base83 decode (4 chars)", lambda: blurhash.base83_decode("L~r:"), 10000)
    print()

    # ------------------------------------------------------------------
    # sRGB / linear conversion
    # ------------------------------------------------------------------
    print("--- sRGB <-> linear ---")
    benchmark(
        "srgb_to_linear (256 values)",
        lambda: [blurhash.srgb_to_linear(i) for i in range(256)],
        1000,
    )
    benchmark(
        "linear_to_srgb (256 values)",
        lambda: [blurhash.linear_to_srgb(i / 255.0) for i in range(256)],
        1000,
    )
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
