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
///
/// Note: Releases the GIL during computation for multi-threaded applications.
#[pyfunction]
#[pyo3(signature = (data, width, height, components_x = 4, components_y = 4))]
fn encode(
    py: Python<'_>,
    data: &[u8],
    width: u32,
    height: u32,
    components_x: u32,
    components_y: u32,
) -> PyResult<String> {
    // Copy data so we can release the GIL safely.
    let data = data.to_vec();
    py.allow_threads(move || {
        blurhash_core::encode(&data, width, height, components_x, components_y).map_err(to_py_err)
    })
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
///
/// Note: Releases the GIL during computation for multi-threaded applications.
#[pyfunction]
#[pyo3(signature = (blurhash, width, height, punch = 1.0))]
fn decode(
    py: Python<'_>,
    blurhash: &str,
    width: u32,
    height: u32,
    punch: f64,
) -> PyResult<Py<PyBytes>> {
    let blurhash = blurhash.to_owned();
    let pixels = py.allow_threads(move || {
        blurhash_core::decode(&blurhash, width, height, punch).map_err(to_py_err)
    })?;
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

/// Encode multiple images into BlurHash strings in one call.
///
/// Releases the GIL during the entire batch, allowing other Python threads to
/// run while all images are being processed.
///
/// Args:
///     items: A list of tuples, each containing (data, width, height, components_x, components_y).
///         - data: Raw pixel bytes in RGB order.
///         - width: Image width in pixels.
///         - height: Image height in pixels.
///         - components_x: Number of horizontal components (1..=9).
///         - components_y: Number of vertical components (1..=9).
///
/// Returns:
///     A list of BlurHash strings, one per input item.
///
/// Raises:
///     ValueError: If any image fails to encode. The error message indicates
///         which item (by index) caused the failure.
#[pyfunction]
fn encode_batch(
    py: Python<'_>,
    items: Vec<(Vec<u8>, u32, u32, u32, u32)>,
) -> PyResult<Vec<String>> {
    py.allow_threads(move || {
        items
            .iter()
            .enumerate()
            .map(|(i, (data, w, h, cx, cy))| {
                blurhash_core::encode(data, *w, *h, *cx, *cy)
                    .map_err(|e| PyValueError::new_err(format!("encode_batch item {}: {}", i, e)))
            })
            .collect::<PyResult<Vec<String>>>()
    })
}

/// Decode multiple BlurHash strings into raw RGB pixel data in one call.
///
/// Releases the GIL during the entire batch, allowing other Python threads to
/// run while all hashes are being decoded.
///
/// Args:
///     items: A list of tuples, each containing (blurhash, width, height, punch).
///         - blurhash: The BlurHash string to decode.
///         - width: Desired output width in pixels.
///         - height: Desired output height in pixels.
///         - punch: Contrast adjustment factor.
///
/// Returns:
///     A list of bytes objects, each containing RGB pixel data for the
///     corresponding input.
///
/// Raises:
///     ValueError: If any hash fails to decode. The error message indicates
///         which item (by index) caused the failure.
#[pyfunction]
fn decode_batch(py: Python<'_>, items: Vec<(String, u32, u32, f64)>) -> PyResult<Vec<Py<PyBytes>>> {
    let results = py.allow_threads(move || {
        items
            .iter()
            .enumerate()
            .map(|(i, (hash, w, h, punch))| {
                blurhash_core::decode(hash, *w, *h, *punch)
                    .map_err(|e| PyValueError::new_err(format!("decode_batch item {}: {}", i, e)))
            })
            .collect::<PyResult<Vec<Vec<u8>>>>()
    })?;
    Ok(results
        .iter()
        .map(|pixels| PyBytes::new(py, pixels).into())
        .collect())
}

/// High-performance BlurHash encoding and decoding (Rust-powered).
#[pymodule]
fn blurhash(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(encode, m)?)?;
    m.add_function(wrap_pyfunction!(decode, m)?)?;
    m.add_function(wrap_pyfunction!(components, m)?)?;
    m.add_function(wrap_pyfunction!(srgb_to_linear, m)?)?;
    m.add_function(wrap_pyfunction!(linear_to_srgb, m)?)?;
    m.add_function(wrap_pyfunction!(encode_batch, m)?)?;
    m.add_function(wrap_pyfunction!(decode_batch, m)?)?;
    Ok(())
}
