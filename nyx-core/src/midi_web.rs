//! WebMIDI backend — talks to `navigator.requestMIDIAccess()` directly
//! via `wasm-bindgen` / `web-sys`.
//!
//! This side-steps an upstream type-inference bug in `midir-0.10.3`'s
//! WebMIDI backend that prevents it from compiling on modern rustc
//! toolchains. The surface exposed here (`open_midi_input`,
//! `open_midi_input_named`, `MidiConnection`) matches the native
//! backend so user code is portable between `cargo build` and
//! `wasm-pack build` without source changes.
//!
//! ### Lifecycle
//!
//! `requestMIDIAccess` returns a JS `Promise`, so opening is inherently
//! asynchronous. [`open_midi_input`] returns a `MidiReceiver` /
//! `MidiConnection` pair **immediately**; the returned receiver starts
//! delivering events once the browser resolves the permission prompt.
//! If the user denies access, or the page is not served over HTTPS /
//! `localhost`, an error is logged to the browser console and no events
//! arrive. The returned receiver stays valid and silent.
//!
//! Dropping the `MidiConnection` clears every input's `onmidimessage`
//! handler before the underlying closures are freed, so stale
//! references in the browser can never call into dropped memory.

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::Reflect;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{MidiAccess, MidiInput, MidiMessageEvent, console};

use crate::midi::{MidiError, MidiReceiver, MidiSender, midi_bridge, parse_midi};

/// One installed `onmidimessage` handler. We keep the `MidiInput` so we
/// can clear its callback on drop, and the `Closure` so wasm-bindgen
/// keeps the underlying trampoline alive.
struct Handler {
    input: MidiInput,
    _closure: Closure<dyn FnMut(MidiMessageEvent)>,
}

/// Shared handler list. The `spawn_local` task pushes into it as it
/// walks the MIDI input map; the `MidiConnection` drops it.
type HandlerList = Rc<RefCell<Vec<Handler>>>;

/// A live WebMIDI connection. MIDI events stop when this is dropped.
pub struct MidiConnection {
    handlers: HandlerList,
}

impl Drop for MidiConnection {
    fn drop(&mut self) {
        // Clear JS-side callbacks before the closures deallocate.
        // (WASM is single-threaded, so no RefCell race with the async
        // setup task is possible.)
        for h in self.handlers.borrow().iter() {
            h.input.set_onmidimessage(None);
        }
    }
}

/// Open the first available MIDI input port (or all of them) and begin
/// forwarding events. See module docs for the async caveat.
pub fn open_midi_input() -> Result<(MidiReceiver, MidiConnection), MidiError> {
    open_midi_input_named(None)
}

/// Open MIDI inputs whose port name contains `name_filter` (case-
/// sensitive substring match). If `None`, every available input is
/// attached.
pub fn open_midi_input_named(
    name_filter: Option<&str>,
) -> Result<(MidiReceiver, MidiConnection), MidiError> {
    let (sender, receiver) = midi_bridge(256);
    let sender: Rc<RefCell<MidiSender>> = Rc::new(RefCell::new(sender));
    let handlers: HandlerList = Rc::new(RefCell::new(Vec::new()));

    let window = web_sys::window()
        .ok_or_else(|| MidiError::Init("no `window` global (non-browser wasm?)".into()))?;
    let nav = window.navigator();

    // `requestMIDIAccess()` returns a JS Promise. The sync-fail path is
    // things like "not a secure context"; the async-fail path is the
    // user denying the permission prompt.
    let promise = nav
        .request_midi_access()
        .map_err(|e| MidiError::Init(format!("requestMIDIAccess unavailable: {e:?}")))?;

    let name_filter = name_filter.map(String::from);
    let sender_task = Rc::clone(&sender);
    let handlers_task = Rc::clone(&handlers);

    spawn_local(async move {
        let access_val = match JsFuture::from(promise).await {
            Ok(v) => v,
            Err(e) => {
                console::error_1(&format!("nyx: MIDI access denied: {e:?}").into());
                return;
            }
        };
        let access: MidiAccess = match access_val.dyn_into() {
            Ok(a) => a,
            Err(_) => {
                console::error_1(&"nyx: expected a MIDIAccess object".into());
                return;
            }
        };

        // MidiInputMap is a JS Map; step through its values via the
        // standard iterator protocol. web-sys doesn't expose a typed
        // iterator for it yet, hence the Reflect-based dance.
        let inputs = access.inputs();
        let values_fn = match Reflect::get(&inputs, &"values".into()) {
            Ok(v) => v,
            Err(_) => return,
        };
        let Ok(values_fn) = values_fn.dyn_into::<js_sys::Function>() else {
            return;
        };
        let iterator_val = match values_fn.call0(&inputs) {
            Ok(v) => v,
            Err(_) => return,
        };
        let Ok(iterator) = iterator_val.dyn_into::<js_sys::Iterator>() else {
            return;
        };

        loop {
            let next = match iterator.next() {
                Ok(n) => n,
                Err(_) => break,
            };
            if next.done() {
                break;
            }
            let Ok(input) = next.value().dyn_into::<MidiInput>() else {
                continue;
            };

            if let Some(filter) = name_filter.as_deref() {
                let name = input.name().unwrap_or_default();
                if !name.contains(filter) {
                    continue;
                }
            }

            // Install an onmidimessage closure for this input. The
            // closure parses raw bytes into a MidiEvent and forwards
            // over the SPSC bridge.
            let sender = Rc::clone(&sender_task);
            // `MidiMessageEvent::data()` returns `Result<Vec<u8>, _>`
            // in this web-sys version. The per-event allocation happens
            // on the JS event-loop thread (not the audio thread), so
            // it's fine — MIDI events come in at human speeds.
            let closure: Closure<dyn FnMut(MidiMessageEvent)> =
                Closure::new(move |event: MidiMessageEvent| {
                    let Ok(bytes) = event.data() else {
                        return;
                    };
                    if let Some(ev) = parse_midi(&bytes) {
                        if let Ok(mut s) = sender.try_borrow_mut() {
                            s.send(ev);
                        }
                    }
                });
            input.set_onmidimessage(Some(closure.as_ref().unchecked_ref()));
            handlers_task.borrow_mut().push(Handler {
                input,
                _closure: closure,
            });
        }
    });

    Ok((receiver, MidiConnection { handlers }))
}
