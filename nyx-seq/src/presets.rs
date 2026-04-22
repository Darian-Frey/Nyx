//! Named synth recipes — ready-to-play voices assembled from Nyx primitives.
//!
//! Every preset is a one-call instrument with opinionated defaults that
//! produce a recognisable sound without configuration. Use them as
//! starting points: drop one into your signal graph, get a useful
//! voice immediately, then reach for the underlying primitives in
//! [`osc`](nyx_core::osc), [`ladder`](nyx_core::ladder), and
//! [`envelope`] if you need finer control.
//!
//! ```ignore
//! use nyx_prelude::*;
//!
//! // Acid bass in 4 lines.
//! let mut bass = presets::tb303(55.0);
//! bass.trigger();
//! // stream bass.next(ctx) into your mix...
//!
//! // Supersaw lead — continuous, gate with your own envelope.
//! let mut lead = presets::supersaw(440.0);
//! let faded = lead.next(ctx) * my_env.next(ctx);
//! ```
//!
//! Each preset is documented with its chosen source/filter/envelope
//! values so readers can use it as a reference implementation when
//! they want to roll their own variant.

use nyx_core::{AudioContext, Signal};

use crate::envelope::{self, Adsr};

/// Band-limited step correction used by every saw/square preset.
/// See [`nyx_core::osc`] for the full explanation.
#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        2.0 * x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + 2.0 * x + 1.0
    } else {
        0.0
    }
}

/// Band-limited sawtooth with external phase state — used by every
/// preset that wants a clean saw at mid-to-high register without
/// allocating a full `SawBl` struct.
#[inline]
fn bl_saw(phase: f32, freq: f32, sr: f32) -> f32 {
    let dt = (freq / sr).abs().min(0.5);
    let naive = 2.0 * phase - 1.0;
    naive - poly_blep(phase, dt)
}

/// Band-limited square with two-discontinuity PolyBLEP correction.
#[inline]
fn bl_square(phase: f32, freq: f32, sr: f32) -> f32 {
    let dt = (freq / sr).abs().min(0.5);
    let naive = if phase < 0.5 { 1.0 } else { -1.0 };
    let shifted = (phase + 0.5).fract();
    naive + poly_blep(phase, dt) - poly_blep(shifted, dt)
}

/// One-pole coefficient `1 − exp(−2π·f/sr)`.
#[inline]
fn one_pole_alpha(cutoff: f32, sr: f32) -> f32 {
    1.0 - (-std::f32::consts::TAU * cutoff / sr).exp()
}

// ─── TB-303 ────────────────────────────────────────────────────────────

/// Roland TB-303-style acid bass: saw → ladder-LP with fast envelope
/// opening the cutoff, high resonance.
pub struct Tb303 {
    phase: f32,
    freq: f32,
    amp_env: Adsr,
    filter_env: Adsr,
    s1: f32,
    s2: f32,
    s3: f32,
    s4: f32,
    last_out: f32,
}

/// Acid-bass preset tuned for the classic TB-303 squelch. High
/// resonance (0.75), envelope-modulated ladder cutoff (400 → 3900 Hz),
/// short amp envelope (20 ms decay).
pub fn tb303(freq: f32) -> Tb303 {
    Tb303 {
        phase: 0.0,
        freq,
        amp_env: envelope::adsr(0.003, 0.20, 0.10, 0.12),
        filter_env: envelope::adsr(0.003, 0.15, 0.00, 0.08),
        s1: 0.0,
        s2: 0.0,
        s3: 0.0,
        s4: 0.0,
        last_out: 0.0,
    }
}

impl Tb303 {
    /// Fire the amplitude + filter envelopes from the attack stage.
    pub fn trigger(&mut self) {
        self.phase = 0.0;
        self.amp_env.trigger();
        self.filter_env.trigger();
    }

    /// Begin the release phase for both envelopes (note-off).
    pub fn release(&mut self) {
        self.amp_env.release();
        self.filter_env.release();
    }

    /// Change the pitch in Hz. Takes effect on the next sample.
    pub fn set_freq(&mut self, freq: f32) {
        self.freq = freq;
    }
}

impl Signal for Tb303 {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let saw = bl_saw(self.phase, self.freq, ctx.sample_rate);
        self.phase += self.freq / ctx.sample_rate;
        self.phase -= self.phase.floor();

        // Ladder with envelope-swept cutoff, high resonance (squelchy).
        let filt = self.filter_env.next(ctx);
        let cutoff = (400.0 + filt * 3500.0).clamp(20.0, ctx.sample_rate * 0.45);
        let g = one_pole_alpha(cutoff, ctx.sample_rate);
        let k = 4.0 * 0.75;
        let u = saw - k * self.last_out.tanh();
        let t_u = u.tanh();
        let t1 = self.s1.tanh();
        let t2 = self.s2.tanh();
        let t3 = self.s3.tanh();
        let t4 = self.s4.tanh();
        self.s1 += g * (t_u - t1);
        self.s2 += g * (t1 - t2);
        self.s3 += g * (t2 - t3);
        self.s4 += g * (t3 - t4);
        self.last_out = self.s4;

        self.s4.tanh() * self.amp_env.next(ctx)
    }
}

// ─── Moog-style bass ──────────────────────────────────────────────────

/// Deep analog-style bass: saw + square mix → ladder LP at a fixed
/// low cutoff, moderate resonance.
pub struct MoogBass {
    saw_phase: f32,
    sqr_phase: f32,
    freq: f32,
    cutoff: f32,
    amp_env: Adsr,
    s1: f32,
    s2: f32,
    s3: f32,
    s4: f32,
    last_out: f32,
}

/// Fat subtractive bass preset. Sawtooth + square mixed at 60/40,
/// ladder LP at 700 Hz with resonance 0.45, slow-ish amp envelope.
pub fn moog_bass(freq: f32) -> MoogBass {
    MoogBass {
        saw_phase: 0.0,
        sqr_phase: 0.0,
        freq,
        cutoff: 700.0,
        amp_env: envelope::adsr(0.010, 0.50, 0.70, 0.30),
        s1: 0.0,
        s2: 0.0,
        s3: 0.0,
        s4: 0.0,
        last_out: 0.0,
    }
}

impl MoogBass {
    pub fn trigger(&mut self) {
        self.saw_phase = 0.0;
        self.sqr_phase = 0.0;
        self.amp_env.trigger();
    }
    pub fn release(&mut self) {
        self.amp_env.release();
    }
    pub fn set_freq(&mut self, freq: f32) {
        self.freq = freq;
    }
    /// Override the filter cutoff in Hz. Defaults to 700.
    pub fn cutoff(mut self, hz: f32) -> Self {
        self.cutoff = hz;
        self
    }
}

impl Signal for MoogBass {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let saw = bl_saw(self.saw_phase, self.freq, ctx.sample_rate);
        let sqr = bl_square(self.sqr_phase, self.freq, ctx.sample_rate);
        self.saw_phase += self.freq / ctx.sample_rate;
        self.saw_phase -= self.saw_phase.floor();
        self.sqr_phase += self.freq / ctx.sample_rate;
        self.sqr_phase -= self.sqr_phase.floor();

        let mixed = saw * 0.60 + sqr * 0.40;

        let cutoff = self.cutoff.clamp(20.0, ctx.sample_rate * 0.45);
        let g = one_pole_alpha(cutoff, ctx.sample_rate);
        let k = 4.0 * 0.45;
        let u = mixed - k * self.last_out.tanh();
        let t_u = u.tanh();
        let t1 = self.s1.tanh();
        let t2 = self.s2.tanh();
        let t3 = self.s3.tanh();
        let t4 = self.s4.tanh();
        self.s1 += g * (t_u - t1);
        self.s2 += g * (t1 - t2);
        self.s3 += g * (t2 - t3);
        self.s4 += g * (t3 - t4);
        self.last_out = self.s4;

        self.s4.tanh() * self.amp_env.next(ctx)
    }
}

// ─── Supersaw ─────────────────────────────────────────────────────────

/// Count of detuned voices used by [`supersaw`].
pub const SUPERSAW_VOICES: usize = 7;

/// Classic 7-voice supersaw lead — the backbone of trance and EDM
/// leads since the JP-8000. No intrinsic envelope: apply your own
/// amplitude control externally (`.amp(adsr)` or similar).
pub struct Supersaw {
    phases: [f32; SUPERSAW_VOICES],
    freq: f32,
}

/// Create a 7-voice supersaw at the given base frequency. Voices are
/// detuned ±18, ±12, ±6, 0 cents around the fundamental with phases
/// staggered to avoid transient alignment.
pub fn supersaw(freq: f32) -> Supersaw {
    Supersaw {
        phases: [0.00, 0.14, 0.29, 0.43, 0.57, 0.71, 0.86],
        freq,
    }
}

impl Supersaw {
    pub fn set_freq(&mut self, freq: f32) {
        self.freq = freq;
    }
}

/// Detune multipliers at ±18, ±12, ±6, 0 cents.
const SUPERSAW_DETUNES: [f32; SUPERSAW_VOICES] = [
    0.98965, 0.99309, 0.99654, 1.00000, 1.00347, 1.00695, 1.01048,
];

impl Signal for Supersaw {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let mut sum = 0.0_f32;
        for (i, &det) in SUPERSAW_DETUNES.iter().enumerate() {
            let f = self.freq * det;
            sum += bl_saw(self.phases[i], f, ctx.sample_rate);
            self.phases[i] += f / ctx.sample_rate;
            self.phases[i] -= self.phases[i].floor();
        }
        // Equal-gain mix scaled by 1/√N keeps perceived loudness
        // roughly matched to a single saw.
        sum / (SUPERSAW_VOICES as f32).sqrt()
    }
}

// ─── Prophet-style pad ────────────────────────────────────────────────

/// Two-oscillator pad: detuned saw + saw one octave below, soft LP,
/// slow envelope. Evokes the Prophet-5 / OB-Xa "warm chord pad".
pub struct ProphetPad {
    phase_a: f32,
    phase_b: f32,
    phase_sub: f32,
    freq: f32,
    amp_env: Adsr,
    lp_state: f32,
}

/// Warm pad at the given root frequency. Two detuned saws (±6 c) plus
/// a saw an octave below, through a soft one-pole LP at 2.8 kHz, with
/// a slow swell envelope.
pub fn prophet_pad(freq: f32) -> ProphetPad {
    ProphetPad {
        phase_a: 0.0,
        phase_b: 0.13,
        phase_sub: 0.0,
        freq,
        amp_env: envelope::adsr(0.40, 0.30, 0.80, 0.60),
        lp_state: 0.0,
    }
}

impl ProphetPad {
    pub fn trigger(&mut self) {
        self.amp_env.trigger();
    }
    pub fn release(&mut self) {
        self.amp_env.release();
    }
    pub fn set_freq(&mut self, freq: f32) {
        self.freq = freq;
    }
}

impl Signal for ProphetPad {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        // Two mildly-detuned saws + sub.
        let det = 1.0035; // ~6 cents
        let sa = bl_saw(self.phase_a, self.freq, ctx.sample_rate);
        let sb = bl_saw(self.phase_b, self.freq * det, ctx.sample_rate);
        let sub = bl_saw(self.phase_sub, self.freq * 0.5, ctx.sample_rate);
        self.phase_a += self.freq / ctx.sample_rate;
        self.phase_a -= self.phase_a.floor();
        self.phase_b += self.freq * det / ctx.sample_rate;
        self.phase_b -= self.phase_b.floor();
        self.phase_sub += self.freq * 0.5 / ctx.sample_rate;
        self.phase_sub -= self.phase_sub.floor();

        let mix = sa * 0.45 + sb * 0.45 + sub * 0.30;

        // Soft LP at 2.8 kHz to kill high harmonics without losing warmth.
        let a = one_pole_alpha(2800.0, ctx.sample_rate);
        self.lp_state += a * (mix - self.lp_state);

        self.lp_state * self.amp_env.next(ctx)
    }
}

// ─── DX7-style bell ───────────────────────────────────────────────────

/// Simple FM bell: sine carrier modulated by a sine at 1.4× the
/// carrier frequency (the inharmonic ratio that gives bell-like
/// metal). Amplitude envelope has no sustain — one-shot percussion.
pub struct Dx7Bell {
    carrier_phase: f32,
    modulator_phase: f32,
    freq: f32,
    index: f32,
    amp_env: Adsr,
    index_env: Adsr,
}

/// DX7-style FM bell preset. Modulator at 1.4× carrier, index swept
/// from full to zero over 400 ms (bright attack, mellow tail),
/// amplitude decays over 1.2 s.
pub fn dx7_bell(freq: f32) -> Dx7Bell {
    Dx7Bell {
        carrier_phase: 0.0,
        modulator_phase: 0.0,
        freq,
        index: 4.0,
        amp_env: envelope::adsr(0.002, 1.20, 0.0, 0.30),
        index_env: envelope::adsr(0.002, 0.40, 0.0, 0.15),
    }
}

impl Dx7Bell {
    pub fn trigger(&mut self) {
        self.carrier_phase = 0.0;
        self.modulator_phase = 0.0;
        self.amp_env.trigger();
        self.index_env.trigger();
    }
    pub fn set_freq(&mut self, freq: f32) {
        self.freq = freq;
    }
}

impl Signal for Dx7Bell {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        const MOD_RATIO: f32 = 1.4;
        let index = self.index * self.index_env.next(ctx);

        // Modulator sine.
        let m = (self.modulator_phase * std::f32::consts::TAU).sin();
        self.modulator_phase += self.freq * MOD_RATIO / ctx.sample_rate;
        self.modulator_phase -= self.modulator_phase.floor();

        // Carrier sine with phase-modulated offset.
        let phase = self.carrier_phase * std::f32::consts::TAU + m * index;
        let c = phase.sin();
        self.carrier_phase += self.freq / ctx.sample_rate;
        self.carrier_phase -= self.carrier_phase.floor();

        c * self.amp_env.next(ctx)
    }
}

// ─── Noise sweep ──────────────────────────────────────────────────────

/// Filtered-noise sweep: the classic build-up riser or cinematic hit.
/// White noise through a bandpass whose centre frequency sweeps up
/// over the note's duration.
pub struct NoiseSweep {
    noise_state: u32,
    duration_samples: f32,
    elapsed: f32,
    amp_env: Adsr,
    // State-variable filter state (2-pole BP).
    svf_low: f32,
    svf_band: f32,
}

/// Create a noise-sweep riser that climbs from 200 Hz → 4 kHz over
/// `duration_secs`. Bandwidth (`Q`) is fixed at 4 for a narrow,
/// pitched-ish sweep.
pub fn noise_sweep(duration_secs: f32) -> NoiseSweep {
    NoiseSweep {
        noise_state: 0xDEAD_BEEF,
        duration_samples: duration_secs.max(0.05),
        elapsed: 0.0,
        amp_env: envelope::adsr(0.002, duration_secs.max(0.05), 0.0, 0.10),
        svf_low: 0.0,
        svf_band: 0.0,
    }
}

impl NoiseSweep {
    pub fn trigger(&mut self) {
        self.elapsed = 0.0;
        self.svf_low = 0.0;
        self.svf_band = 0.0;
        self.amp_env.trigger();
    }
}

impl Signal for NoiseSweep {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        // White noise source.
        let mut x = self.noise_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise_state = x;
        let noise = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;

        // Sweep progress 0 → 1.
        let total = self.duration_samples * ctx.sample_rate;
        let t = (self.elapsed / total).min(1.0);
        self.elapsed += 1.0;

        // Exponential sweep 200 → 4000 Hz.
        let cutoff = 200.0 * (20.0_f32).powf(t);

        // 2-pole state-variable bandpass.
        let f = 2.0 * (std::f32::consts::PI * cutoff / ctx.sample_rate).sin();
        let q = 1.0 / 4.0; // fixed Q = 4
        let high = noise - self.svf_low - q * self.svf_band;
        self.svf_band += f * high;
        self.svf_low += f * self.svf_band;

        self.svf_band * self.amp_env.next(ctx)
    }
}

// ─── Juno-style pad (PWM) ─────────────────────────────────────────────

/// Juno-60-ish warm pad: two detuned PWM voices whose pulse width is
/// slowly swept by an LFO (the classic "juno chorus" sound comes from
/// the PW movement itself). Slow-attack sustained envelope.
pub struct JunoPad {
    phase_a: f32,
    phase_b: f32,
    freq: f32,
    // Shared LFO for pulse-width modulation.
    lfo_phase: f32,
    amp_env: Adsr,
    lp_state: f32,
}

/// Warm PWM pad at the given root frequency. Two voices detuned ±6
/// cents, each PWM-modulated by a 0.3 Hz LFO sweeping width 0.35 ↔
/// 0.65, one-pole LP at 3.5 kHz, slow-swell envelope.
pub fn juno_pad(freq: f32) -> JunoPad {
    JunoPad {
        phase_a: 0.0,
        phase_b: 0.17,
        freq,
        lfo_phase: 0.0,
        amp_env: envelope::adsr(0.35, 0.30, 0.80, 0.60),
        lp_state: 0.0,
    }
}

impl JunoPad {
    pub fn trigger(&mut self) {
        self.amp_env.trigger();
    }
    pub fn release(&mut self) {
        self.amp_env.release();
    }
    pub fn set_freq(&mut self, freq: f32) {
        self.freq = freq;
    }
}

impl Signal for JunoPad {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        // Shared LFO → pulse width in [0.35, 0.65].
        const LFO_RATE_HZ: f32 = 0.3;
        let lfo = (self.lfo_phase * std::f32::consts::TAU).sin();
        self.lfo_phase += LFO_RATE_HZ / ctx.sample_rate;
        self.lfo_phase -= self.lfo_phase.floor();
        let width_a = 0.5 + 0.15 * lfo;
        // Second voice uses inverted LFO for gentle stereo-like motion.
        let width_b = 0.5 - 0.15 * lfo;

        // Detune: ±6 cents.
        const DET_A: f32 = 0.99654;
        const DET_B: f32 = 1.00347;

        let pulse = |phase: f32, freq: f32, width: f32| -> f32 {
            let dt = (freq / ctx.sample_rate).abs().min(0.5);
            let naive = if phase < width { 1.0 } else { -1.0 };
            let shifted = (phase - width + 1.0).fract();
            naive + poly_blep(phase, dt) - poly_blep(shifted, dt)
        };

        let pa = pulse(self.phase_a, self.freq * DET_A, width_a);
        let pb = pulse(self.phase_b, self.freq * DET_B, width_b);
        self.phase_a += self.freq * DET_A / ctx.sample_rate;
        self.phase_a -= self.phase_a.floor();
        self.phase_b += self.freq * DET_B / ctx.sample_rate;
        self.phase_b -= self.phase_b.floor();

        let mix = (pa + pb) * 0.5;

        // One-pole LP at 3.5 kHz — softens the pulse edges without
        // losing the PWM character.
        let a = one_pole_alpha(3500.0, ctx.sample_rate);
        self.lp_state += a * (mix - self.lp_state);

        self.lp_state * self.amp_env.next(ctx)
    }
}

// ─── Modal synthesis voices ───────────────────────────────────────────

/// Number of partials used by [`Handpan`] and [`Chime`]. Four partials
/// is enough for recognisable tuned-metal character without being
/// expensive (one sin + one mul per partial per sample plus a shared
/// decay-step table).
pub const MODAL_PARTIALS: usize = 4;

/// Recursive exponential decay: `damp_step = exp(−1 / (tau · sr))`.
/// Computed once at trigger, applied each sample as a single multiply.
/// Rewriting `exp(−t/tau)` as a running product avoids per-sample
/// `exp()` calls in the audio loop.
#[inline]
fn decay_step(tau_secs: f32, sr: f32) -> f32 {
    if tau_secs <= 0.0 {
        return 0.0;
    }
    (-1.0 / (tau_secs * sr)).exp()
}

/// Tuned steel drum / handpan. Four exponentially-damped sine
/// partials at approximately 1, 2, 3.01, 5.03 × fundamental.
pub struct Handpan {
    phases: [f32; MODAL_PARTIALS],
    damps: [f32; MODAL_PARTIALS],
    decay_steps: [f32; MODAL_PARTIALS],
    freq: f32,
    sr: f32,
    initialised: bool,
}

/// Partial ratios / relative amplitudes / decay τ (seconds) for the
/// handpan preset — tuned by ear to sit between "steel pan" and
/// "handpan / hang drum".
const HANDPAN_RATIOS: [f32; MODAL_PARTIALS] = [1.00, 2.00, 3.01, 5.03];
const HANDPAN_AMPS: [f32; MODAL_PARTIALS] = [0.70, 0.32, 0.15, 0.08];
const HANDPAN_TAUS: [f32; MODAL_PARTIALS] = [1.50, 0.90, 0.45, 0.20];

/// Create a handpan voice at the given fundamental frequency.
pub fn handpan(freq: f32) -> Handpan {
    Handpan {
        phases: [0.0; MODAL_PARTIALS],
        damps: [0.0; MODAL_PARTIALS],
        decay_steps: [0.0; MODAL_PARTIALS],
        freq,
        sr: 0.0,
        initialised: false,
    }
}

impl Handpan {
    pub fn trigger(&mut self) {
        // Reset phases so each strike starts coherent.
        self.phases = [0.0; MODAL_PARTIALS];
        self.damps = [1.0; MODAL_PARTIALS];
    }
    pub fn set_freq(&mut self, freq: f32) {
        self.freq = freq;
    }
}

impl Signal for Handpan {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        if !self.initialised || self.sr != ctx.sample_rate {
            self.sr = ctx.sample_rate;
            for (dst, &tau) in self.decay_steps.iter_mut().zip(HANDPAN_TAUS.iter()) {
                *dst = decay_step(tau, ctx.sample_rate);
            }
            self.initialised = true;
        }

        let mut sum = 0.0_f32;
        for i in 0..MODAL_PARTIALS {
            let s = (self.phases[i] * std::f32::consts::TAU).sin();
            sum += HANDPAN_AMPS[i] * self.damps[i] * s;
            self.phases[i] += self.freq * HANDPAN_RATIOS[i] / ctx.sample_rate;
            self.phases[i] -= self.phases[i].floor();
            self.damps[i] *= self.decay_steps[i];
        }
        sum
    }
}

/// Tubular-bell / chime. Modal synthesis with longer decays and more
/// inharmonic partial ratios than [`Handpan`] — picks up the "ringing
/// metal" character that FM approximations can't quite reach.
///
/// *Not* to be confused with the existing [`Dx7Bell`]: that one uses
/// FM, this one uses summed damped sines.
pub struct Chime {
    phases: [f32; MODAL_PARTIALS],
    damps: [f32; MODAL_PARTIALS],
    decay_steps: [f32; MODAL_PARTIALS],
    freq: f32,
    sr: f32,
    initialised: bool,
}

/// Bell / chime partial set — approximate tubular-bell spectrum.
/// Hum tone (0.5×) sustains longest; the perceived pitch ~2× sits
/// an octave above the hum and decays faster.
const CHIME_RATIOS: [f32; MODAL_PARTIALS] = [0.50, 1.19, 2.00, 3.00];
const CHIME_AMPS: [f32; MODAL_PARTIALS] = [0.45, 0.35, 0.55, 0.20];
const CHIME_TAUS: [f32; MODAL_PARTIALS] = [3.50, 1.60, 2.20, 0.90];

/// Modal-synthesis bell / chime at the given fundamental frequency.
pub fn chime(freq: f32) -> Chime {
    Chime {
        phases: [0.0; MODAL_PARTIALS],
        damps: [0.0; MODAL_PARTIALS],
        decay_steps: [0.0; MODAL_PARTIALS],
        freq,
        sr: 0.0,
        initialised: false,
    }
}

impl Chime {
    pub fn trigger(&mut self) {
        self.phases = [0.0; MODAL_PARTIALS];
        self.damps = [1.0; MODAL_PARTIALS];
    }
    pub fn set_freq(&mut self, freq: f32) {
        self.freq = freq;
    }
}

impl Signal for Chime {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        if !self.initialised || self.sr != ctx.sample_rate {
            self.sr = ctx.sample_rate;
            for (dst, &tau) in self.decay_steps.iter_mut().zip(CHIME_TAUS.iter()) {
                *dst = decay_step(tau, ctx.sample_rate);
            }
            self.initialised = true;
        }

        let mut sum = 0.0_f32;
        for i in 0..MODAL_PARTIALS {
            let s = (self.phases[i] * std::f32::consts::TAU).sin();
            sum += CHIME_AMPS[i] * self.damps[i] * s;
            self.phases[i] += self.freq * CHIME_RATIOS[i] / ctx.sample_rate;
            self.phases[i] -= self.phases[i].floor();
            self.damps[i] *= self.decay_steps[i];
        }
        sum
    }
}
