use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

/// Convert a `BlurhashError` into a Python `ValueError`.
fn to_py_err(e: blurhash_core::BlurhashError) -> PyErr {
    PyValueError::new_err(e.to_string())
}

/// Encode image pixel data into a BlurHash string.
///
/// Args:
///     data: Raw pixel bytes in RGB order (length must be width * height * 3).
///     width: Image width in pixels.
///     height: Image height in pixels.
///     components_x: Number of horizontal components (1..=9).
///     components_y: Number of vertical components (1..=9).
///
/// Returns:
///     The BlurHash string.
#[pyfunction]
#[pyo3(signature = (data, width, height, components_x = 4, components_y = 4))]
fn encode(
    data: &[u8],
    width: u32,
    height: u32,
    components_x: u32,
    components_y: u32,
) -> PyResult<String> {
    blurhash_core::encode(data, width, height, components_x, components_y).map_err(to_py_err)
}

/// Decode a BlurHash string into raw RGB pixel data.
///
/// Args:
///     blurhash: The BlurHash string to decode.
///     width: Desired output width in pixels (1..=10000).
///     height: Desired output height in pixels (1..=10000).
///     punch: Contrast adjustment factor (default 1.0).
///
/// Returns:
///     A bytes object of length width * height * 3 containing RGB pixel data.
#[pyfunction]
#[pyo3(signature = (blurhash, width, height, punch = 1.0))]
fn decode(
    py: Python<'_>,
    blurhash: &str,
    width: u32,
    height: u32,
    punch: f64,
) -> PyResult<Py<PyBytes>> {
    let pixels = blurhash_core::decode(blurhash, width, height, punch).map_err(to_py_err)?;
    Ok(PyBytes::new(py, &pixels).into())
}

/// Extract the number of X and Y components from a BlurHash string.
///
/// Args:
///     blurhash: The BlurHash string.
///
/// Returns:
///     A tuple (components_x, components_y).
#[pyfunction]
fn components(blurhash: &str) -> PyResult<(u32, u32)> {
    blurhash_core::components(blurhash).map_err(to_py_err)
}

/// Convert an sRGB byte value (0-255) to linear RGB (0.0-1.0).
#[pyfunction]
fn srgb_to_linear(value: u8) -> f64 {
    blurhash_core::color::srgb_to_linear(value)
}

/// Convert a linear RGB value (0.0-1.0) to an sRGB byte value (0-255).
#[pyfunction]
fn linear_to_srgb(value: f64) -> u8 {
    blurhash_core::color::linear_to_srgb(value)
}

/// High-performance BlurHash encoding and decoding (Rust-powered).
#[pymodule]
fn blurhash(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(encode, m)?)?;
    m.add_function(wrap_pyfunction!(decode, m)?)?;
    m.add_function(wrap_pyfunction!(components, m)?)?;
    m.add_function(wrap_pyfunction!(srgb_to_linear, m)?)?;
    m.add_function(wrap_pyfunction!(linear_to_srgb, m)?)?;
    Ok(())
}
