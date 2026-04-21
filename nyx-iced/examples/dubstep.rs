//! DUBSTEP SHOWCASE
//!
//! A full 32-bar dubstep performance built entirely from Nyx primitives,
//! with a live oscilloscope, spectrum analyser, and section/beat display.
//!
//! Track structure (loops every ~55 seconds at 140 BPM):
//!   Intro      — bars 0..4   — pad + filtered hats
//!   Build 1    — bars 4..8   — snare roll + riser + kicks
//!   DROP 1     — bars 8..16  — wobble bass, sub, full drums
//!   Breakdown  — bars 16..20 — pad + sub, filter sweep
//!   Build 2    — bars 20..24 — intense snare roll
//!   DROP 2     — bars 24..32 — growl bass, lead melody, all drums
//!
//! Shows off: Clock + ClockState, Euclidean rhythms, Pattern<bool>/Pattern<u8>,
//! Sequence<T>, Note/Scale/Chord, ADSR envelopes, seeded RNG, inst::{kick,
//! snare, hihat}, SubSynth, inline biquad filter for fast LFO sweeps,
//! multi-voice mixing, soft clipping, atomic cross-thread state, scope +
//! spectrum visualisers, and the iced GUI.
//!
//! Run: cargo run -p nyx-iced --example dubstep --release

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use iced::widget::{column, container, row, text};
use iced::{Element, Length, Subscription, Theme};

use nyx_core::osc_input::OscParam;
use nyx_core::{
    Engine, ScopeExt, ScopeHandle, Signal, SpectrumConfig, SpectrumExt, SpectrumHandle,
};
use nyx_iced::{Knob, KnobMessage, KnobState, OscilloscopeCanvas, SpectrumCanvas};
use nyx_seq::inst::{HiHat, Kick, Snare};
use nyx_seq::{
    Adsr, Chord, ChordType, Clock, ClockState, Euclid, Note, Pattern, Scale, Sequence, SubSynth,
    SynthPatch, clock, envelope, inst, seeded,
};

const BPM: f32 = 140.0;

// ─── Section enum (stored as AtomicU32 for GUI) ──────────────────────
const SEC_INTRO: u32 = 0;
const SEC_BUILD1: u32 = 1;
const SEC_DROP1: u32 = 2;
const SEC_BREAKDOWN: u32 = 3;
const SEC_BUILD2: u32 = 4;
const SEC_DROP2: u32 = 5;

fn section_for_bar(bar: i32) -> u32 {
    match bar.rem_euclid(32) {
        0..=3 => SEC_INTRO,
        4..=7 => SEC_BUILD1,
        8..=15 => SEC_DROP1,
        16..=19 => SEC_BREAKDOWN,
        20..=23 => SEC_BUILD2,
        _ => SEC_DROP2,
    }
}

fn section_name(s: u32) -> &'static str {
    match s {
        SEC_INTRO => "INTRO",
        SEC_BUILD1 => "BUILD 1",
        SEC_DROP1 => "DROP 1",
        SEC_BREAKDOWN => "BREAKDOWN",
        SEC_BUILD2 => "BUILD 2",
        SEC_DROP2 => "DROP 2",
        _ => "?",
    }
}

// ─── Shared atomic state ─────────────────────────────────────────────
#[derive(Clone)]
struct TrackInfo {
    section: Arc<AtomicU32>,
    bar: Arc<AtomicU32>,
    beat_in_bar: Arc<AtomicU32>,
}

impl TrackInfo {
    fn new() -> Self {
        Self {
            section: Arc::new(AtomicU32::new(SEC_INTRO)),
            bar: Arc::new(AtomicU32::new(0)),
            beat_in_bar: Arc::new(AtomicU32::new(0)),
        }
    }
}

// ─── The Track Signal ────────────────────────────────────────────────
struct DubstepTrack {
    clk: Clock<nyx_core::param::ConstSignal>,
    info: TrackInfo,
    last_bar: i32,

    // Drums
    kick: Kick,
    snare: Snare,
    hat_c: HiHat,
    hat_o: HiHat,

    // Sequences (all 16-step, sixteenth-note grid)
    kick_seq: Sequence<bool>,
    snare_seq: Sequence<bool>,
    hat_seq: Sequence<bool>,

    // Sub bass — pure sine at E1 (41.2 Hz)
    sub_phase: f32,
    sub_env: Adsr,

    // Wobble bass: detuned saws through inline resonant LP (fast moving cutoff)
    wob_saw1: f32,
    wob_saw2: f32,
    wob_lfo: f32,
    wob_s1: f32,
    wob_s2: f32,

    // Growl bass: square + saw, gritty tanh → LP
    growl_saw: f32,
    growl_sq: f32,
    growl_lfo: f32,
    growl_s1: f32,
    growl_s2: f32,

    // Pad — 4-voice sine stack on Cmaj7 (E♭m7 transposed for the drop)
    pad_phases: [f32; 4],
    pad_freqs_intro: [f32; 4],
    pad_freqs_drop: [f32; 4],
    pad_env: Adsr,

    // Lead melody — SubSynth playing a pentatonic figure in DROP 2
    lead: SubSynth,
    lead_notes: [Note; 8],
    lead_grid_index: i64,

    // Riser (filtered noise, rises through builds)
    riser_state: u32,
    riser_phase: f32,

    // Master gain param (wired to the GUI knob)
    master_gain: nyx_core::osc_input::OscSignal,
}

impl DubstepTrack {
    fn new(info: TrackInfo, master_gain: nyx_core::osc_input::OscSignal) -> Self {
        // Drum patterns — 16 sixteenths = 1 bar
        let kick_pat = Pattern::new(&[
            true, false, false, false, false, false, false, false, // beat 1
            false, false, false, true, false, false, false, false, // kick on 3.75 for swagger
        ]);
        let snare_pat = Pattern::new(&[
            false, false, false, false, false, false, false, false, true, false, false, false,
            false, false, false, false, // beat 3
        ]);
        // Euclidean hats — 11 in 16 gives a driving groove
        let hat_pat = Euclid::generate(11, 16);

        // Pad chord: C minor 7 (the classic dubstep mood) — intro octave higher
        let intro_chord = Chord::new(Note::C4, ChordType::Minor7);
        let drop_chord = Chord::new(Note::from_midi(48), ChordType::Minor7); // C3
        let pad_freqs_intro = freqs_4(&intro_chord);
        let pad_freqs_drop = freqs_4(&drop_chord);

        // Lead — seeded pentatonic figure
        let mut rng = seeded(0xD1B_57EB);
        let scale = Scale::pentatonic_minor("C");
        let lead_notes: [Note; 8] = [
            rng.next_note_in(&scale, Note::from_midi(60), Note::from_midi(72)),
            rng.next_note_in(&scale, Note::from_midi(60), Note::from_midi(72)),
            rng.next_note_in(&scale, Note::from_midi(60), Note::from_midi(72)),
            rng.next_note_in(&scale, Note::from_midi(60), Note::from_midi(72)),
            rng.next_note_in(&scale, Note::from_midi(60), Note::from_midi(72)),
            rng.next_note_in(&scale, Note::from_midi(60), Note::from_midi(72)),
            rng.next_note_in(&scale, Note::from_midi(60), Note::from_midi(72)),
            rng.next_note_in(&scale, Note::from_midi(60), Note::from_midi(72)),
        ];

        let lead_patch = SynthPatch {
            name: "Lead".into(),
            osc_shape: nyx_seq::OscShape::Square,
            frequency: 440.0,
            filter_type: nyx_seq::FilterType::LowPass,
            filter_cutoff: 2500.0,
            filter_q: 3.0,
            attack: 0.001,
            decay: 0.08,
            sustain: 0.2,
            release: 0.05,
            gain: 0.35,
        };

        Self {
            clk: clock::clock(BPM),
            info,
            last_bar: -1,
            kick: inst::kick(),
            snare: inst::snare(),
            hat_c: inst::hihat(false),
            hat_o: inst::hihat(true),
            kick_seq: Sequence::new(kick_pat, 0.25),
            snare_seq: Sequence::new(snare_pat, 0.25),
            hat_seq: Sequence::new(hat_pat, 0.25),
            sub_phase: 0.0,
            sub_env: envelope::adsr(0.005, 0.2, 0.9, 0.1),
            wob_saw1: 0.0,
            wob_saw2: 0.0,
            wob_lfo: 0.0,
            wob_s1: 0.0,
            wob_s2: 0.0,
            growl_saw: 0.0,
            growl_sq: 0.0,
            growl_lfo: 0.0,
            growl_s1: 0.0,
            growl_s2: 0.0,
            pad_phases: [0.0; 4],
            pad_freqs_intro,
            pad_freqs_drop,
            pad_env: envelope::adsr(0.8, 0.2, 0.8, 1.2),
            lead: lead_patch.build(),
            lead_notes,
            lead_grid_index: -1,
            riser_state: 0xCAFE_D00D,
            riser_phase: 0.0,
            master_gain,
        }
    }
}

fn freqs_4(chord: &Chord) -> [f32; 4] {
    let freqs = chord.freqs();
    let mut out = [0.0; 4];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = freqs[i % freqs.len()];
    }
    out
}

// ─── Inline resonant biquad lowpass (per-sample parameter changes) ───
fn biquad_lp(cutoff: f32, q: f32, sr: f32, s1: &mut f32, s2: &mut f32, input: f32) -> f32 {
    let omega = std::f32::consts::TAU * cutoff.clamp(20.0, sr * 0.45) / sr;
    let sin_w = omega.sin();
    let cos_w = omega.cos();
    let alpha = sin_w / (2.0 * q.max(0.5));
    let a0 = 1.0 + alpha;
    let b0 = ((1.0 - cos_w) / 2.0) / a0;
    let b1 = (1.0 - cos_w) / a0;
    let b2 = b0;
    let a1 = (-2.0 * cos_w) / a0;
    let a2 = (1.0 - alpha) / a0;
    let out = b0 * input + *s1;
    *s1 = b1 * input - a1 * out + *s2;
    *s2 = b2 * input - a2 * out;
    out
}

impl Signal for DubstepTrack {
    fn next(&mut self, ctx: &nyx_core::AudioContext) -> f32 {
        let state: ClockState = self.clk.tick(ctx);
        let bar = state.bar as i32;
        let beat_in_bar = (state.beat as i32).rem_euclid(4) as u32;

        // Publish to GUI on bar / beat transitions
        if bar != self.last_bar {
            self.last_bar = bar;
            let section = section_for_bar(bar);
            self.info.section.store(section, Ordering::Relaxed);
            self.info.bar.store(bar as u32, Ordering::Relaxed);
        }
        self.info.beat_in_bar.store(beat_in_bar, Ordering::Relaxed);

        let section = self.info.section.load(Ordering::Relaxed);

        // ─── Drum triggering ──────────────────────────────────────────
        let k = self.kick_seq.tick(&state);
        let s = self.snare_seq.tick(&state);
        let h = self.hat_seq.tick(&state);

        // Kicks in all sections except intro and breakdown
        if k.triggered && k.value {
            match section {
                SEC_INTRO | SEC_BREAKDOWN => {}
                _ => self.kick.trigger(),
            }
        }
        // Snare on beat 3 in drops; snare roll in builds
        let roll_bar = matches!(section, SEC_BUILD1 | SEC_BUILD2);
        if s.triggered {
            if roll_bar {
                // Snare roll: every sixteenth for BUILD2, every 2nd for BUILD1
                let div: u32 = if section == SEC_BUILD2 { 1 } else { 2 };
                if (s.step as u32).is_multiple_of(div) || s.value {
                    self.snare.trigger();
                }
            } else if s.value && !matches!(section, SEC_INTRO | SEC_BREAKDOWN) {
                self.snare.trigger();
            }
        }
        // Extra accents on kick grid positions during builds
        if roll_bar && k.triggered {
            let div: u32 = if section == SEC_BUILD2 { 1 } else { 2 };
            if (k.step as u32).is_multiple_of(div) {
                self.snare.trigger();
            }
        }
        // Hats — filtered during intro, open variety during drops
        if h.triggered && h.value {
            if matches!(section, SEC_DROP1 | SEC_DROP2) && (h.step % 4 == 2) {
                self.hat_o.trigger(); // open hat on off-beats
            } else {
                self.hat_c.trigger();
            }
        }

        // Lead melody — only in DROP 2, triggered on every 8th note
        if matches!(section, SEC_DROP2) {
            let gi = (state.beat * 2.0) as i64;
            if gi != self.lead_grid_index {
                self.lead_grid_index = gi;
                let idx = (gi as usize) % self.lead_notes.len();
                self.lead.set_frequency(self.lead_notes[idx].to_freq());
                self.lead.trigger();
            }
        }

        // Sub bass envelope: on during drops, off otherwise
        let sub_active = matches!(section, SEC_DROP1 | SEC_DROP2 | SEC_BREAKDOWN);
        if sub_active && k.triggered && k.value {
            self.sub_env.trigger();
        }
        if !sub_active {
            self.sub_env.release();
        }

        // Pad envelope: on in intro/breakdown
        let pad_active = matches!(section, SEC_INTRO | SEC_BREAKDOWN);
        if pad_active && k.triggered && state.beat.fract() < 0.01 && beat_in_bar == 0 {
            self.pad_env.trigger();
        }
        if !pad_active {
            self.pad_env.release();
        }

        // ─── Sample generation ────────────────────────────────────────

        // Sub (sine at E1)
        let sub_freq = 41.2; // E1
        let sub = (self.sub_phase * std::f32::consts::TAU).sin();
        self.sub_phase += sub_freq / ctx.sample_rate;
        self.sub_phase -= self.sub_phase.floor();
        let sub_env = self.sub_env.next(ctx);
        let sub_sample = sub * sub_env * 0.7;

        // Wobble bass LFO — rate depends on section
        let wob_rate = match section {
            SEC_DROP1 => 2.0, // half-note wobble at 140bpm → 2Hz-ish
            SEC_DROP2 => 4.0, // faster quarter-note wobble
            SEC_BREAKDOWN => 0.5,
            _ => 0.0, // no wobble outside drops
        };
        self.wob_lfo += wob_rate / ctx.sample_rate;
        self.wob_lfo -= self.wob_lfo.floor();
        let wob_lfo_val = (self.wob_lfo * std::f32::consts::TAU).sin() * 0.5 + 0.5;
        let wob_cutoff = 150.0 + wob_lfo_val * 2200.0;

        // Wobble saws at E1 (41.2) and detuned
        let wob_freq = 41.2;
        let wob_in = (2.0 * self.wob_saw1 - 1.0) + 0.6 * (2.0 * self.wob_saw2 - 1.0);
        self.wob_saw1 += wob_freq / ctx.sample_rate;
        self.wob_saw1 -= self.wob_saw1.floor();
        self.wob_saw2 += (wob_freq * 1.005) / ctx.sample_rate;
        self.wob_saw2 -= self.wob_saw2.floor();
        let wob_filtered = biquad_lp(
            wob_cutoff,
            3.5,
            ctx.sample_rate,
            &mut self.wob_s1,
            &mut self.wob_s2,
            wob_in * 0.5,
        );
        let wob_sample = (wob_filtered * 2.5).tanh() * 0.6; // soft clip for grit

        // Growl bass — only in DROP 2. Square + saw, heavier.
        let growl_freq = 41.2;
        self.growl_lfo += 6.0 / ctx.sample_rate;
        self.growl_lfo -= self.growl_lfo.floor();
        let growl_lfo_val = (self.growl_lfo * std::f32::consts::TAU).sin().abs();
        let growl_cutoff = 200.0 + growl_lfo_val * 1800.0;
        let sq = if self.growl_sq < 0.5 { 1.0 } else { -1.0 };
        let saw = 2.0 * self.growl_saw - 1.0;
        let growl_in = 0.6 * sq + 0.4 * saw;
        self.growl_sq += growl_freq / ctx.sample_rate;
        self.growl_sq -= self.growl_sq.floor();
        self.growl_saw += (growl_freq * 2.003) / ctx.sample_rate;
        self.growl_saw -= self.growl_saw.floor();
        let growl_filtered = biquad_lp(
            growl_cutoff,
            5.0,
            ctx.sample_rate,
            &mut self.growl_s1,
            &mut self.growl_s2,
            growl_in * 0.5,
        );
        let growl_sample = if section == SEC_DROP2 {
            (growl_filtered * 4.0).tanh() * 0.55
        } else {
            0.0
        };

        // Pad — four-voice sine stack
        let pad_freqs = if section == SEC_INTRO {
            &self.pad_freqs_intro
        } else {
            &self.pad_freqs_drop
        };
        let mut pad = 0.0_f32;
        for (phase, &freq) in self.pad_phases.iter_mut().zip(pad_freqs.iter()) {
            pad += (*phase * std::f32::consts::TAU).sin();
            *phase += freq / ctx.sample_rate;
            *phase -= phase.floor();
        }
        let pad_env = self.pad_env.next(ctx);
        let pad_sample = (pad * 0.25) * pad_env;

        // Drums
        let kick_sample = self.kick.next(ctx);
        let snare_sample = self.snare.next(ctx);
        let hat_sample = self.hat_c.next(ctx) * 0.4 + self.hat_o.next(ctx) * 0.3;

        // Lead (SubSynth) — rendered unconditionally so envelope runs
        let lead_sample = self.lead.next(ctx) * if section == SEC_DROP2 { 1.0 } else { 0.0 };

        // Riser — filtered noise that ramps in during builds
        let mut r = self.riser_state;
        r ^= r << 13;
        r ^= r >> 17;
        r ^= r << 5;
        self.riser_state = r;
        let riser_noise = (r as f32 / u32::MAX as f32) * 2.0 - 1.0;
        self.riser_phase += 1.0 / ctx.sample_rate;
        let riser_amp = if roll_bar {
            // Ramp from 0 to ~0.4 over the 4-bar build
            let build_start = if section == SEC_BUILD1 { 4 } else { 20 };
            let bars_in_build = (bar - build_start) as f32;
            let t = (bars_in_build + state.phase_in_bar) / 4.0;
            t.clamp(0.0, 1.0) * 0.35
        } else {
            0.0
        };
        let riser_sample = riser_noise * riser_amp;

        // ─── Mix ──────────────────────────────────────────────────────
        let mix = match section {
            SEC_INTRO => pad_sample + hat_sample * 0.4,
            SEC_BUILD1 => kick_sample * 0.8 + snare_sample * 0.5 + hat_sample * 0.7 + riser_sample,
            SEC_DROP1 => {
                kick_sample + snare_sample + hat_sample * 0.8 + wob_sample + sub_sample * 0.8
            }
            SEC_BREAKDOWN => pad_sample * 0.8 + sub_sample * 0.4 + hat_sample * 0.3,
            SEC_BUILD2 => kick_sample + snare_sample + hat_sample + riser_sample * 1.4,
            SEC_DROP2 => {
                kick_sample
                    + snare_sample
                    + hat_sample
                    + wob_sample * 0.7
                    + growl_sample
                    + sub_sample
                    + lead_sample * 0.6
            }
            _ => 0.0,
        };

        // Master bus: soft clip + gain (from GUI knob)
        let gain = self.master_gain.next(ctx);
        (mix * 0.6).tanh() * gain
    }
}

// ─── iced app ────────────────────────────────────────────────────────
fn main() -> iced::Result {
    iced::application("Nyx — Dubstep", App::update, App::view)
        .subscription(App::subscription)
        .theme(|_| Theme::Dark)
        .window_size((1000.0, 700.0))
        .run_with(App::new)
}

struct App {
    _engine: Engine,
    scope_handle: ScopeHandle,
    spec_handle: SpectrumHandle,
    oscilloscope: OscilloscopeCanvas,
    spectrum_view: SpectrumCanvas,
    gain: KnobState,
    gain_param: OscParam,
    info: TrackInfo,
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    Gain(KnobMessage),
}

impl App {
    fn new() -> (Self, iced::Task<Message>) {
        let info = TrackInfo::new();
        let gain_param = OscParam::new(0.6);
        let gain_sig = gain_param.signal(5.0);

        let track = DubstepTrack::new(info.clone(), gain_sig);

        // Tap scope and spectrum at the master output
        let (sig, scope_handle) = track.scope(8192);
        let (sig, spec_handle) = sig.spectrum(SpectrumConfig {
            frame_size: 2048,
            ..Default::default()
        });

        let engine = Engine::play(sig).expect("failed to open audio device");

        (
            App {
                _engine: engine,
                scope_handle,
                spec_handle,
                oscilloscope: OscilloscopeCanvas::new(2048).width(900.0).height(200.0),
                spectrum_view: SpectrumCanvas::new(96).width(900.0).height(200.0),
                gain: KnobState::new(0.6),
                gain_param,
                info,
            },
            iced::Task::none(),
        )
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Tick => {
                self.oscilloscope.update(&mut self.scope_handle);
                self.spectrum_view.update(&self.spec_handle);
            }
            Message::Gain(KnobMessage::Changed(v)) => {
                self.gain.value = v;
                self.gain_param.writer().set(v);
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let section = self.info.section.load(Ordering::Relaxed);
        let bar = self.info.bar.load(Ordering::Relaxed);
        let beat = self.info.beat_in_bar.load(Ordering::Relaxed);

        let header = row![
            text(format!("▓▒░ {} ░▒▓", section_name(section))).size(28),
            text(format!("bar {:02}   beat {}", bar + 1, beat + 1)).size(18),
        ]
        .spacing(40);

        let scope = self.oscilloscope.view().map(|_| Message::Tick);
        let spectrum = self.spectrum_view.view().map(|_| Message::Tick);

        let knob = Knob::new(&self.gain).size(72.0).view().map(Message::Gain);
        let gain_label = text(format!("Master  {:.0}%", self.gain.value * 100.0)).size(14);

        let controls = row![column![gain_label, knob].spacing(6)].spacing(20);

        container(
            column![
                header,
                text("Oscilloscope").size(13),
                scope,
                text("Spectrum").size(13),
                spectrum,
                controls,
            ]
            .spacing(10)
            .padding(18),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        iced::time::every(std::time::Duration::from_millis(33)).map(|_| Message::Tick)
    }
}
