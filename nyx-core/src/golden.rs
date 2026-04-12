//! Golden-file DSP regression test framework.
//!
//! Renders a `Signal` to a buffer and compares the result against a stored
//! binary file in `tests/golden/`. If the file doesn't exist, it is created
//! (first run or regeneration). On subsequent runs, the output is compared
//! sample-by-sample with a configurable tolerance.
//!
//! # Regenerating golden files
//!
//! Set `NYX_UPDATE_GOLDEN=1` to overwrite all golden files with current output.

use crate::render::render_to_buffer;
use crate::signal::Signal;
use std::path::Path;

/// Configuration for a golden-file test.
pub struct GoldenTest {
    pub name: &'static str,
    pub duration_secs: f32,
    pub sample_rate: f32,
    pub tolerance: f32,
}

impl Default for GoldenTest {
    fn default() -> Self {
        Self {
            name: "unnamed",
            duration_secs: 0.1,
            sample_rate: 44100.0,
            tolerance: 1e-6,
        }
    }
}

/// Run a golden-file comparison for the given signal.
///
/// - If the golden file doesn't exist or `NYX_UPDATE_GOLDEN=1`, writes it.
/// - Otherwise, loads the file and compares sample-by-sample.
///
/// # Panics
///
/// Panics if any sample differs by more than `config.tolerance`.
pub fn assert_golden(signal: &mut dyn Signal, config: &GoldenTest) {
    let buf = render_to_buffer(signal, config.duration_secs, config.sample_rate);

    let golden_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let golden_path = golden_dir.join(format!("{}.bin", config.name));

    let should_update = std::env::var("NYX_UPDATE_GOLDEN")
        .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));

    if should_update || !golden_path.exists() {
        std::fs::create_dir_all(&golden_dir).expect("failed to create tests/golden/");
        let bytes: Vec<u8> = buf.iter().flat_map(|s| s.to_le_bytes()).collect();
        std::fs::write(&golden_path, bytes)
            .unwrap_or_else(|e| panic!("failed to write golden file {}: {e}", golden_path.display()));
        eprintln!(
            "nyx golden: wrote {} samples to {}",
            buf.len(),
            golden_path.display()
        );
        return;
    }

    let bytes = std::fs::read(&golden_path)
        .unwrap_or_else(|e| panic!("failed to read golden file {}: {e}", golden_path.display()));

    let expected: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
        .collect();

    assert_eq!(
        buf.len(),
        expected.len(),
        "golden file {}: sample count mismatch (got {}, expected {})",
        config.name,
        buf.len(),
        expected.len()
    );

    for (i, (got, exp)) in buf.iter().zip(expected.iter()).enumerate() {
        let diff = (got - exp).abs();
        assert!(
            diff <= config.tolerance,
            "golden file {}: sample {} differs by {diff} (got {got}, expected {exp}, tolerance {})",
            config.name,
            i,
            config.tolerance
        );
    }
}
