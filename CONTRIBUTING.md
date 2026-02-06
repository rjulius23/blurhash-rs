# Contributing to blurhash-rs

Thank you for your interest in contributing to blurhash-rs. This guide covers how to set up your development environment, run tests and benchmarks, and submit changes.

---

## Development Setup

### Prerequisites

- **Rust 1.70+** -- install via [rustup](https://rustup.rs/)
- **Python 3.8+** -- for Python bindings development
- **Node.js 18+** -- for TypeScript bindings development
- **maturin** -- for building Python bindings (`pip install maturin`)

### Clone and build

```bash
git clone https://github.com/rjulius23/blurhash-rs
cd blurhash-rs

# Build the entire workspace
cargo build

# Run all Rust tests
cargo test
```

### Python bindings

```bash
cd bindings/python

# Create a virtual environment (recommended)
python -m venv .venv
source .venv/bin/activate  # or .venv\Scripts\activate on Windows

# Build and install in development mode
pip install maturin
maturin develop

# Verify
python -c "import blurhash; print('Python bindings OK')"
```

### TypeScript bindings

```bash
cd bindings/typescript

# Install dependencies
npm install

# Build native addon
npm run build

# Verify
node -e "const b = require('.'); console.log('TypeScript bindings OK')"
```

---

## Running Tests

### Rust core

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run a specific test
cargo test test_name
```

### Python

```bash
cd bindings/python
maturin develop
pytest
```

### TypeScript

```bash
cd bindings/typescript
npm run build
npm test
```

---

## Running Benchmarks

### Rust benchmarks (Criterion)

```bash
# Run all benchmarks
cargo bench

# Run a specific benchmark
cargo bench --bench blurhash_bench

# Generate HTML report (opens in browser)
# Reports are saved to target/criterion/
cargo bench --bench blurhash_bench
open target/criterion/report/index.html
```

### Cross-language comparison

Compare Rust performance against the Python reference:

```bash
# Run the Python reference implementation
cd reference
python -c "
import time
from blurhash_python_original import blurhash_encode, blurhash_decode

# Create test image
image = [[[128, 64, 32] for x in range(128)] for y in range(128)]

start = time.perf_counter()
for _ in range(10):
    blurhash_encode(image, 4, 4)
elapsed = (time.perf_counter() - start) / 10
print(f'Python encode 128x128: {elapsed*1000:.1f} ms')
"
```

---

## Project Structure

```
blurhash-rs/
  crates/
    blurhash-core/         # Core Rust library
      src/
        lib.rs             # Public API
        encode.rs          # Encoding implementation
        decode.rs          # Decoding implementation
        base83.rs          # Base83 codec
        color.rs           # Color space conversions
        error.rs           # Error types
  bindings/
    python/                # PyO3 bindings
    typescript/            # napi-rs bindings
  benchmarks/              # Benchmark scripts
  reference/               # Original Python source
  docs/                    # Design documents
```

---

## Making Changes

### Code style

- Run `cargo fmt` before committing.
- Run `cargo clippy` and fix any warnings.
- Follow existing code conventions and naming patterns.

### Correctness

This library must produce **byte-identical output** to the original Python implementation. If you modify the encoding or decoding logic:

1. Run the cross-validation test suite to confirm output matches.
2. Test edge cases: 1x1 components, 9x9 components, 1x1 images, non-square images.
3. Test both `linear=True` and `linear=False` modes.

### Performance

If your change affects the hot path (encoding, decoding, base83, color conversion):

1. Run `cargo bench` before and after your change.
2. Include benchmark results in your PR description.
3. Regressions need justification.

---

## Pull Request Process

1. **Fork** the repository and create a feature branch from `main`.
2. **Make your changes** with clear, focused commits.
3. **Run the full test suite:** `cargo test && cargo clippy && cargo fmt --check`
4. **Open a PR** against `main` with:
   - A clear description of what changed and why.
   - Benchmark results if performance-relevant.
   - Test results confirming correctness.
5. A maintainer will review your PR. Address any feedback.
6. Once approved, your PR will be merged.

---

## Reporting Issues

- Use [GitHub Issues](https://github.com/rjulius23/blurhash-rs/issues) for bug reports and feature requests.
- For bugs, include: platform, language (Rust/Python/TypeScript), version, and a minimal reproduction.
- For performance issues, include benchmark output and hardware details.

---

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
