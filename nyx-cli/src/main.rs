mod sketch;
mod watcher;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use clap::Parser;
use nyx_core::hotswap::HotSwap;
use nyx_core::{Engine, EngineConfig, Signal};

/// Nyx audio sketch player with live hot-reload.
#[derive(Parser)]
#[command(name = "nyx", about = "Nyx audio sketch player")]
struct Cli {
    /// Path to a .rs sketch file to play.
    sketch: PathBuf,

    /// Watch the file and hot-reload on save.
    #[arg(short, long, default_value_t = true)]
    watch: bool,

    /// Crossfade duration in milliseconds when hot-reloading.
    #[arg(long, default_value_t = 50.0)]
    crossfade_ms: f32,

    /// Sample rate in Hz.
    #[arg(long, default_value_t = 44100)]
    sample_rate: u32,

    /// Buffer size in samples.
    #[arg(long, default_value_t = 512)]
    buffer_size: u32,
}

fn main() {
    let cli = Cli::parse();

    if !cli.sketch.exists() {
        eprintln!("nyx: sketch file not found: {}", cli.sketch.display());
        std::process::exit(1);
    }

    let target_dir = std::env::temp_dir().join("nyx-live");

    // Initial compile
    println!("nyx: compiling {}...", cli.sketch.display());
    let start = Instant::now();
    let lib_path = match sketch::compile_sketch(&cli.sketch, &target_dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("nyx: {e}");
            std::process::exit(1);
        }
    };
    println!("nyx: compiled in {:.1}s", start.elapsed().as_secs_f32());

    // Load
    let (signal, _loaded) = match sketch::load_sketch(&lib_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("nyx: {e}");
            std::process::exit(1);
        }
    };

    // Wrap in HotSwap for live reloading
    let sr = cli.sample_rate as f32;
    let hotswap = Arc::new(Mutex::new(HotSwap::new(signal, cli.crossfade_ms, sr)));

    // Create a signal adapter that locks the hotswap briefly per buffer
    // (NOT per sample — see the engine callback).
    let hotswap_clone = Arc::clone(&hotswap);
    let adapter = HotSwapAdapter {
        hotswap: hotswap_clone,
    };

    // Start audio
    let config = EngineConfig {
        sample_rate: cli.sample_rate,
        buffer_size: cli.buffer_size,
        channels: 2,
    };
    let _engine = match Engine::play_with(adapter, config) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("nyx: audio error: {e}");
            std::process::exit(1);
        }
    };

    println!("nyx: playing — {} ", cli.sketch.display());

    if cli.watch {
        println!("nyx: watching for changes (save to hot-reload, Ctrl+C to quit)");

        let (rx, _watcher) = match watcher::watch_file(&cli.sketch) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("nyx: watch error: {e}");
                std::process::exit(1);
            }
        };

        // Keep track of the loaded library to prevent it being unloaded.
        let mut _current_lib = _loaded;

        loop {
            // Block until file change.
            if rx.recv().is_err() {
                break;
            }
            // Debounce: drain any queued events.
            while rx.try_recv().is_ok() {}

            println!("nyx: change detected, recompiling...");
            let start = Instant::now();

            match sketch::compile_sketch(&cli.sketch, &target_dir) {
                Ok(new_lib_path) => {
                    let elapsed = start.elapsed().as_secs_f32();
                    match sketch::load_sketch(&new_lib_path) {
                        Ok((new_signal, new_lib)) => {
                            if let Ok(mut hs) = hotswap.lock() {
                                hs.swap(new_signal);
                            }
                            _current_lib = new_lib;
                            println!("nyx: reloaded in {elapsed:.1}s");
                        }
                        Err(e) => eprintln!("nyx: load error: {e}"),
                    }
                }
                Err(e) => eprintln!("nyx: {e}"),
            }
        }
    } else {
        println!("nyx: press Enter to stop");
        let mut buf = String::new();
        let _ = std::io::stdin().read_line(&mut buf);
    }
}

/// Adapter that wraps a `Mutex<HotSwap>` as a `Signal`.
///
/// The mutex is only held for the duration of one `next()` call.
/// This is acceptable because the audio thread is the only reader
/// and the main thread only writes briefly during `swap()`.
struct HotSwapAdapter {
    hotswap: Arc<Mutex<HotSwap>>,
}

impl Signal for HotSwapAdapter {
    fn next(&mut self, ctx: &nyx_core::AudioContext) -> f32 {
        if let Ok(mut hs) = self.hotswap.try_lock() {
            hs.next(ctx)
        } else {
            // Contention during swap — output silence for one sample.
            0.0
        }
    }
}

// Safety: the Arc<Mutex> is Send.
unsafe impl Send for HotSwapAdapter {}
