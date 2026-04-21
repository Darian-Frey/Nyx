//! Sketch compiler and dynamic library loader.
//!
//! Compiles a `.rs` sketch file to a cdylib, then loads it and calls
//! the `nyx_sketch` entry point to get a `Box<dyn Signal>`.

use std::path::{Path, PathBuf};
use std::process::Command;

use libloading::{Library, Symbol};
use nyx_core::Signal;

/// A loaded sketch library.
pub struct LoadedSketch {
    _library: Library,
    // Keep the library alive so the function pointers remain valid.
}

/// The function signature that sketch cdylibs must export.
///
/// ```ignore
/// #[no_mangle]
/// pub fn nyx_sketch() -> Box<dyn nyx_core::Signal> {
///     osc::sine(440.0).boxed()
/// }
/// ```
type SketchFn = unsafe fn() -> Box<dyn Signal>;

/// Compile a sketch `.rs` file to a cdylib.
///
/// Returns the path to the compiled `.so` / `.dylib` / `.dll`.
pub fn compile_sketch(sketch_path: &Path, target_dir: &Path) -> Result<PathBuf, SketchError> {
    let sketch_path = sketch_path
        .canonicalize()
        .map_err(|e| SketchError::Io(format!("cannot resolve sketch path: {e}")))?;

    // The sketch is compiled as a standalone cdylib crate.
    // We generate a temporary Cargo.toml that depends on nyx-core and nyx-seq,
    // then cargo build it.
    let sketch_dir = target_dir.join("nyx-sketch-build");
    let src_dir = sketch_dir.join("src");
    std::fs::create_dir_all(&src_dir).map_err(|e| SketchError::Io(format!("mkdir: {e}")))?;

    // Copy the sketch file as lib.rs
    std::fs::copy(&sketch_path, src_dir.join("lib.rs"))
        .map_err(|e| SketchError::Io(format!("copy sketch: {e}")))?;

    // Find the workspace root (parent of nyx-cli)
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("nyx-cli should be inside the workspace");

    // Generate Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "nyx-sketch"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
nyx-core = {{ path = "{}/nyx-core", default-features = false }}
nyx-seq = {{ path = "{}/nyx-seq" }}
nyx-prelude = {{ path = "{}/nyx-prelude", default-features = false }}
"#,
        workspace_root.display(),
        workspace_root.display(),
        workspace_root.display(),
    );
    std::fs::write(sketch_dir.join("Cargo.toml"), cargo_toml)
        .map_err(|e| SketchError::Io(format!("write Cargo.toml: {e}")))?;

    // Compile
    let output = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--manifest-path")
        .arg(sketch_dir.join("Cargo.toml"))
        .output()
        .map_err(|e| SketchError::Compile(format!("failed to run cargo: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SketchError::Compile(stderr.to_string()));
    }

    // Find the built library
    let lib_name = if cfg!(target_os = "macos") {
        "libnyx_sketch.dylib"
    } else if cfg!(target_os = "windows") {
        "nyx_sketch.dll"
    } else {
        "libnyx_sketch.so"
    };

    let lib_path = sketch_dir.join("target").join("release").join(lib_name);

    if !lib_path.exists() {
        return Err(SketchError::Compile(format!(
            "compiled library not found at {}",
            lib_path.display()
        )));
    }

    Ok(lib_path)
}

/// Load a compiled sketch library and call its entry point.
pub fn load_sketch(lib_path: &Path) -> Result<(Box<dyn Signal>, LoadedSketch), SketchError> {
    // Copy the library to a unique path to avoid OS caching issues on reload.
    let unique_path = lib_path.with_extension(format!(
        "{}.so",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::copy(lib_path, &unique_path)
        .map_err(|e| SketchError::Io(format!("copy lib for reload: {e}")))?;

    let library = unsafe {
        Library::new(&unique_path).map_err(|e| SketchError::Load(format!("dlopen: {e}")))?
    };

    let signal = unsafe {
        let func: Symbol<SketchFn> = library
            .get(b"nyx_sketch")
            .map_err(|e| SketchError::Load(format!("symbol 'nyx_sketch' not found: {e}")))?;
        func()
    };

    Ok((signal, LoadedSketch { _library: library }))
}

/// Errors from sketch compilation and loading.
#[derive(Debug)]
pub enum SketchError {
    Io(String),
    Compile(String),
    Load(String),
}

impl std::fmt::Display for SketchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SketchError::Io(e) => write!(f, "I/O error: {e}"),
            SketchError::Compile(e) => write!(f, "compilation error:\n{e}"),
            SketchError::Load(e) => write!(f, "load error: {e}"),
        }
    }
}

impl std::error::Error for SketchError {}
