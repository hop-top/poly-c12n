use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use c12n_core::{ClassificationContext, Pipeline, PipelineResult as RustPipelineResult};
use std::collections::HashMap;
use std::time::Duration;

/// Python wrapper around `c12n_core::Pipeline`.
#[pyclass]
struct PyPipeline {
    pipeline: Pipeline,
    runtime: tokio::runtime::Runtime,
}

#[pymethods]
impl PyPipeline {
    #[new]
    #[pyo3(signature = (max_concurrency=8, timeout_ms=5000))]
    fn new(max_concurrency: usize, timeout_ms: u64) -> PyResult<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| PyRuntimeError::new_err(format!("failed to create runtime: {e}")))?;

        let pipeline = Pipeline::new(vec![], max_concurrency, Duration::from_millis(timeout_ms));

        Ok(Self { pipeline, runtime })
    }

    /// Evaluate the pipeline and return a `PyPipelineResult`.
    ///
    /// The returned object wraps a JSON-serialised result that can be
    /// retrieved via its `.json()` method.
    ///
    /// `config_json` accepts an optional JSON string that will be parsed
    /// into the classification context's config map.
    #[pyo3(signature = (text, history=None, headers=None, image_url=None, config_json=None))]
    fn evaluate(
        &self,
        text: String,
        history: Option<Vec<String>>,
        headers: Option<HashMap<String, String>>,
        image_url: Option<String>,
        config_json: Option<String>,
    ) -> PyResult<PyPipelineResult> {
        let config: HashMap<String, serde_json::Value> = match config_json {
            Some(s) => serde_json::from_str(&s)
                .map_err(|e| PyRuntimeError::new_err(format!("invalid config JSON: {e}")))?,
            None => HashMap::new(),
        };

        let ctx = ClassificationContext {
            text,
            history: history.unwrap_or_default(),
            headers: headers.unwrap_or_default(),
            image_url,
            config,
        };

        let result = self
            .runtime
            .block_on(self.pipeline.evaluate(&ctx));

        serialize_result(result)
    }

    /// No-op — reserved for future resource cleanup.
    fn close(&mut self) -> PyResult<()> {
        Ok(())
    }
}

/// Serialized pipeline result exposed to Python.
#[pyclass]
#[derive(Clone)]
struct PyPipelineResult {
    json_str: String,
}

#[pymethods]
impl PyPipelineResult {
    /// Return raw JSON string of the full result.
    fn json(&self) -> &str {
        &self.json_str
    }

    fn __repr__(&self) -> String {
        format!("PyPipelineResult({})", &self.json_str)
    }
}

fn serialize_result(result: RustPipelineResult) -> PyResult<PyPipelineResult> {
    let out = serde_json::json!({
        "results": result.results,
        "errors": result.errors.iter().map(|e| e.to_string()).collect::<Vec<_>>(),
        "duration_ms": result.duration.as_millis() as u64,
    });

    let json_str = serde_json::to_string(&out)
        .map_err(|e| PyRuntimeError::new_err(format!("serialization failed: {e}")))?;

    Ok(PyPipelineResult { json_str })
}

/// c12n — Classification engine Python bindings.
#[pymodule]
fn c12n(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPipeline>()?;
    m.add_class::<PyPipelineResult>()?;
    Ok(())
}
