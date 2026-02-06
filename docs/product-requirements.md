# blurhash-rs Product Requirements

## 1. Product Vision

BlurHash is a compact representation of a placeholder for an image, widely used in production applications to display aesthetically pleasing placeholders while full images load. The existing Python implementation (`blurhash-python`) is functionally correct but too slow for server-side use at scale -- encoding and decoding are CPU-bound operations dominated by nested loops and trigonometric calculations that Python handles poorly.

**blurhash-rs** is a ground-up Rust port of the blurhash algorithm, distributed as:

- A **Python package** (`pip install blurhash-rs`) via PyO3 + maturin, providing a drop-in replacement for `blurhash-python` with identical API signatures.
- A **Node.js/TypeScript package** (`npm install blurhash-rs`) via napi-rs, giving the JavaScript ecosystem a high-performance native blurhash implementation.

By moving the core computation to Rust, we target a **minimum 100x performance improvement** over the Python reference for both encode and decode operations. This unlocks use cases that are impractical today: real-time encoding in web servers, batch processing in image pipelines, and low-latency decoding in server-side rendering.

---

## 2. User Stories

### Python Developers Migrating from blurhash-python

**US-1: Drop-in replacement for existing Python code**
> As a Python developer currently using `blurhash-python`, I want to switch to `blurhash-rs` by only changing my import statement, so that I get a massive performance improvement without rewriting any application code.

Acceptance: `import blurhash_rs as blurhash` works as a drop-in replacement. All public functions (`blurhash_encode`, `blurhash_decode`, `blurhash_components`) accept the same arguments and return the same types as the original.

**US-2: Identical output for existing workloads**
> As a Python developer, I want `blurhash-rs` to produce byte-identical blurhash strings for the same input images, so that I can migrate without breaking cached hashes or visual regressions.

Acceptance: Given the same pixel data, component counts, and parameters, the Rust port produces the exact same blurhash string as the Python reference implementation.

### TypeScript / Node.js Developers

**US-3: Native npm package for Node.js**
> As a Node.js developer building an image service, I want to `npm install blurhash-rs` and call `encode`/`decode` from TypeScript, so that I can generate blurhash placeholders on the server without spawning child processes or using slow JS implementations.

Acceptance: The npm package exports `encode`, `decode`, and `components` functions callable from both JavaScript and TypeScript with full type definitions included.

**US-4: TypeScript type safety**
> As a TypeScript developer, I want the npm package to ship with `.d.ts` type declarations, so that I get autocomplete and compile-time type checking.

Acceptance: The package includes TypeScript declarations for all exported functions. `tsc --noEmit` passes when importing and using the package.

### Performance-Sensitive Applications

**US-5: High-throughput encoding in web servers**
> As a backend engineer running an image upload service, I want blurhash encoding to complete in under 1ms for typical images (e.g., 256x256 with 4x4 components), so that I can compute hashes inline during upload without adding perceptible latency.

Acceptance: Encoding a 256x256 image with 4x4 components completes in under 1ms on modern hardware (M1/x86-64). Benchmarked at >= 100x faster than the Python reference.

**US-6: Batch decoding in image pipelines**
> As a data engineer building an image processing pipeline, I want to decode thousands of blurhash strings per second into pixel arrays, so that I can generate placeholder images at scale for CDN pre-warming.

Acceptance: Decoding a blurhash to a 32x32 pixel grid completes in under 100 microseconds. Throughput exceeds 10,000 decodes/second on a single core.

**US-7: Thread-safe concurrent usage**
> As a developer using async frameworks (FastAPI, Express), I want blurhash-rs to be safe to call from multiple threads concurrently, so that I can use it in multi-threaded server environments without locks or synchronization.

Acceptance: All exported functions are thread-safe. The Python package releases the GIL during Rust computation. The Node.js package supports concurrent calls via napi-rs async/threadsafe patterns.

### DevOps and Packaging

**US-8: Cross-platform binary wheels and npm prebuilds**
> As a DevOps engineer, I want pre-built binary packages for Linux (x64, arm64), macOS (x64, arm64), and Windows (x64), so that `pip install` and `npm install` work without requiring a Rust toolchain on the target machine.

Acceptance: CI publishes pre-built wheels to PyPI and prebuilt binaries to npm for all six platform targets. Installation does not require `cargo`, `rustc`, or any C compiler.

**US-9: Minimal dependency footprint**
> As a platform engineer, I want `blurhash-rs` to have zero runtime dependencies beyond the language runtime (Python / Node.js), so that it does not introduce supply-chain risk or version conflicts.

Acceptance: The Rust core has no third-party runtime dependencies. The Python wheel lists no additional pip dependencies. The npm package lists no production `dependencies`.

---

## 3. Acceptance Criteria

### 3.1 API Compatibility (Python)

| Python Reference Function | Signature | blurhash-rs Must Match |
|---|---|---|
| `blurhash_encode(image, components_x=4, components_y=4, linear=False)` | Accepts 3D list of pixel values (height x width x 3), returns blurhash string | Identical signature, identical return value |
| `blurhash_decode(blurhash, width, height, punch=1.0, linear=False)` | Accepts blurhash string and dimensions, returns 3D list of pixel values | Identical signature, identical return value |
| `blurhash_components(blurhash)` | Accepts blurhash string, returns `(size_x, size_y)` tuple | Identical signature, identical return value |

- All parameter names, default values, and types must match.
- Error behavior must match: `ValueError` raised for the same invalid inputs (blurhash too short, invalid length, component counts out of 1-9 range).

### 3.2 API Surface (Node.js / TypeScript)

| Function | Signature |
|---|---|
| `encode(pixels: Uint8Array, width: number, height: number, componentX?: number, componentY?: number)` | Returns blurhash string |
| `decode(blurhash: string, width: number, height: number, punch?: number)` | Returns `Uint8Array` of RGB pixel data |
| `components(blurhash: string)` | Returns `{ x: number, y: number }` |

- The Node.js API uses flat `Uint8Array` buffers (standard for image data in JS) rather than nested arrays.
- TypeScript `.d.ts` declarations are included in the package.

### 3.3 Performance

| Operation | Input | Target | Measurement |
|---|---|---|---|
| Encode | 256x256 image, 4x4 components | >= 100x faster than Python reference | Wall-clock time, single-threaded benchmark |
| Decode | Blurhash string to 32x32 pixels | >= 100x faster than Python reference | Wall-clock time, single-threaded benchmark |
| Encode | 1024x1024 image, 4x4 components | >= 100x faster than Python reference | Wall-clock time, single-threaded benchmark |

- Benchmarks must be reproducible via a script included in the repository.
- Benchmarks run on both x86-64 and arm64 (Apple Silicon) in CI.

### 3.4 Correctness

- All blurhash strings produced by `blurhash_encode` for a given input must be **byte-identical** to the Python reference output.
- All pixel arrays produced by `blurhash_decode` for a given blurhash must be **value-identical** to the Python reference output (integer RGB values match exactly).
- The `blurhash_components` function returns identical tuples.
- Edge cases tested:
  - Minimum components (1x1)
  - Maximum components (9x9)
  - 1x1 pixel image
  - Non-square images (wide and tall aspect ratios)
  - `linear=True` mode for both encode and decode
  - `punch` parameter values other than 1.0
  - Invalid inputs raise appropriate errors

### 3.5 Packaging and Distribution

| Target | Package Name | Registry | Platforms |
|---|---|---|---|
| Python | `blurhash-rs` | PyPI | linux-x64, linux-arm64, macos-x64, macos-arm64, windows-x64 |
| Node.js | `blurhash-rs` | npm | linux-x64, linux-arm64, macos-x64, macos-arm64, windows-x64 |

- Python: maturin-built wheels, supporting Python 3.8+.
- Node.js: napi-rs prebuilds, supporting Node.js 18+ (current LTS and later).
- Source distributions available as fallback for unsupported platforms.
- `pip install blurhash-rs` and `npm install blurhash-rs` work without a Rust toolchain.

### 3.6 Quality

- Rust core achieves zero `unsafe` blocks (or documents and justifies any that exist).
- `cargo clippy` passes with no warnings.
- `cargo test` passes with 100% of tests green.
- Python tests run via `pytest` with a test suite covering all acceptance criteria.
- Node.js tests run via a standard test runner (vitest or jest).

---

## 4. Success Metrics

| Metric | Target | Timeframe |
|---|---|---|
| Performance multiplier vs Python reference (encode) | >= 100x | At launch |
| Performance multiplier vs Python reference (decode) | >= 100x | At launch |
| Python API compatibility | 100% function signature and output parity | At launch |
| Test vector pass rate | 100% (all reference test cases produce identical output) | At launch |
| Cross-platform wheel/prebuild coverage | 5/5 targets (linux-x64, linux-arm64, macos-x64, macos-arm64, win-x64) | At launch |
| CI pipeline green | All builds, tests, and benchmarks pass on every supported platform | At launch |

---

## 5. Risks and Mitigations

| Risk | Impact | Likelihood | Mitigation |
|---|---|---|---|
| **Floating-point divergence** between Rust and Python causes output mismatches | High -- breaks drop-in compatibility promise | Medium | Use `f64` throughout Rust core (matching Python's float). Build a comprehensive cross-validation test suite comparing Rust output to Python output for diverse inputs. Investigate and match Python's `math.floor`/rounding behavior exactly. |
| **napi-rs build complexity** across platforms | Medium -- npm install fails on some targets | Medium | Use napi-rs GitHub Actions matrix builds. Test all five platform targets in CI. Provide source-build fallback instructions. |
| **PyO3/maturin ABI compatibility** with multiple Python versions | Medium -- wheels fail on older/newer Python | Low | Build wheels for Python 3.8-3.13 using maturin's abi3 stable ABI support, reducing the matrix to a single wheel per platform. |
| **Performance target not met** on specific hardware | Medium -- marketing claim weakened | Low | The Python reference is extremely slow due to nested Python loops; even a naive Rust translation should exceed 100x. Profile and optimize hot loops (base83 encode/decode, DCT computation). Consider SIMD for the trigonometric inner loop if needed. |
| **Name collision on PyPI/npm** | High -- cannot publish under desired name | Low | Check `blurhash-rs` availability on both registries early. Have fallback names ready (`blurhash-rust`, `blurhash-native`). |
| **Thread-safety issues** in Python GIL release | High -- crashes in multi-threaded servers | Low | Ensure all Rust functions are `Send + Sync`. Test under concurrent load with Python `threading` and `concurrent.futures`. Use PyO3's `allow_threads` correctly. |
| **Large binary size** in distributed wheels/prebuilds | Low -- slower install, larger containers | Medium | Use `opt-level = "z"` and `lto = true` in release profile. Strip debug symbols. Target < 2MB per platform binary. |
