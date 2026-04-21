//! Nyx GUI demo: oscilloscope, spectrum, knob, sliders, and XY pad
//! all wired to a live audio signal.
//!
//! Controls:
//!   Knob      → master volume
//!   H-Slider  → oscillator frequency (50–2000 Hz)
//!   V-Slider  → filter resonance (Q: 0.5–10)
//!   XY Pad    → X = filter cutoff (100–8000 Hz), Y = detune amount
//!
//! Run with: cargo run -p nyx-iced --example demo

use iced::widget::{column, container, row, text};
use iced::{Element, Length, Subscription, Theme};

use nyx_core::filter::FilterExt;
use nyx_core::osc_input::OscParam;
use nyx_core::{
    Engine, ScopeExt, ScopeHandle, Signal, SignalExt, SpectrumConfig, SpectrumExt, SpectrumHandle,
};
use nyx_iced::{
    HSlider, Knob, KnobMessage, KnobState, OscilloscopeCanvas, SliderMessage, SliderState,
    SpectrumCanvas, VSlider, XYPad, XYPadMessage, XYPadState,
};

fn main() -> iced::Result {
    iced::application("Nyx Demo", App::update, App::view)
        .subscription(App::subscription)
        .theme(|_| Theme::Dark)
        .window_size((900.0, 600.0))
        .run_with(App::new)
}

struct App {
    _engine: Engine,
    scope_handle: ScopeHandle,
    spectrum_handle: SpectrumHandle,
    oscilloscope: OscilloscopeCanvas,
    spectrum_view: SpectrumCanvas,
    // Controls
    knob: KnobState,
    hslider: SliderState,
    vslider: SliderState,
    xypad: XYPadState,
    // Atomic parameters → audio thread
    gain_param: OscParam,
    freq_param: OscParam,
    cutoff_param: OscParam,
    q_param: OscParam,
    detune_param: OscParam,
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    Knob(KnobMessage),
    HSlider(SliderMessage),
    VSlider(SliderMessage),
    XYPad(XYPadMessage),
}

impl App {
    fn new() -> (Self, iced::Task<Message>) {
        // Atomic params shared between GUI and audio thread.
        let gain_param = OscParam::new(0.3);
        let freq_param = OscParam::new(220.0);
        let cutoff_param = OscParam::new(2000.0);
        let q_param = OscParam::new(0.707);
        let detune_param = OscParam::new(1.003);

        // Build audio signal using smoothed atomic readers.
        let gain_sig = gain_param.signal(5.0);
        let freq_sig = freq_param.signal(5.0);
        let cutoff_sig = cutoff_param.signal(5.0);
        let q_sig = q_param.signal(5.0);
        let detune_sig = detune_param.signal(5.0);

        // Two detuned saws through a resonant lowpass, controlled by params.
        let sig = DetuneOsc {
            phase1: 0.0,
            phase2: 0.0,
            freq: freq_sig,
            detune: detune_sig,
        }
        .lowpass(cutoff_sig, q_sig)
        .amp(gain_sig);

        let (sig, scope_handle) = sig.scope(4096);
        let (sig, spectrum_handle) = sig.spectrum(SpectrumConfig {
            frame_size: 2048,
            ..Default::default()
        });

        let engine = Engine::play(sig).expect("failed to open audio device");

        (
            App {
                _engine: engine,
                scope_handle,
                spectrum_handle,
                oscilloscope: OscilloscopeCanvas::new(1024).width(420.0).height(180.0),
                spectrum_view: SpectrumCanvas::new(64).width(420.0).height(180.0),
                knob: KnobState::new(0.5),
                hslider: SliderState::new(0.5),
                vslider: SliderState::new(0.5),
                xypad: XYPadState::new(0.5, 0.5),
                gain_param,
                freq_param,
                cutoff_param,
                q_param,
                detune_param,
            },
            iced::Task::none(),
        )
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Tick => {
                self.oscilloscope.update(&mut self.scope_handle);
                self.spectrum_view.update(&self.spectrum_handle);
            }
            Message::Knob(KnobMessage::Changed(v)) => {
                self.knob.value = v;
                // Knob → master gain (0.0–0.8)
                self.gain_param.writer().set(v * 0.8);
            }
            Message::HSlider(SliderMessage::Changed(v)) => {
                self.hslider.value = v;
                // H-Slider → frequency (50–2000 Hz, exponential mapping)
                let freq = 50.0 * (2000.0_f32 / 50.0).powf(v);
                self.freq_param.writer().set(freq);
            }
            Message::VSlider(SliderMessage::Changed(v)) => {
                self.vslider.value = v;
                // V-Slider → filter Q (0.5–10)
                let q = 0.5 + v * 9.5;
                self.q_param.writer().set(q);
            }
            Message::XYPad(XYPadMessage::Changed { x, y }) => {
                self.xypad.x = x;
                self.xypad.y = y;
                // X → filter cutoff (100–8000 Hz, exponential)
                let cutoff = 100.0 * (8000.0_f32 / 100.0).powf(x);
                self.cutoff_param.writer().set(cutoff);
                // Y → detune amount (1.0–1.02)
                let detune = 1.0 + y * 0.02;
                self.detune_param.writer().set(detune);
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let scope = self.oscilloscope.view().map(|_| Message::Tick);
        let spectrum = self.spectrum_view.view().map(|_| Message::Tick);

        let freq_hz = 50.0 * (2000.0_f32 / 50.0).powf(self.hslider.value);
        let cutoff_hz = 100.0 * (8000.0_f32 / 100.0).powf(self.xypad.x);
        let q_val = 0.5 + self.vslider.value * 9.5;

        let visualisers = column![
            text("Oscilloscope").size(14),
            scope,
            text("Spectrum").size(14),
            spectrum,
        ]
        .spacing(8);

        let knob_widget = Knob::new(&self.knob).size(80.0).view().map(Message::Knob);
        let hslider_widget = HSlider::new(&self.hslider)
            .width(200.0)
            .view()
            .map(Message::HSlider);
        let vslider_widget = VSlider::new(&self.vslider)
            .height(150.0)
            .view()
            .map(Message::VSlider);
        let xypad_widget = XYPad::new(&self.xypad)
            .size(150.0)
            .view()
            .map(Message::XYPad);

        let controls = column![
            text(format!("Volume: {:.0}%", self.knob.value * 100.0)).size(14),
            knob_widget,
            text(format!("Frequency: {freq_hz:.0} Hz")).size(14),
            hslider_widget,
            row![
                column![
                    text(format!("Resonance: {q_val:.1}")).size(14),
                    vslider_widget,
                ]
                .spacing(4),
                column![
                    text(format!("Cutoff: {cutoff_hz:.0} Hz")).size(14),
                    xypad_widget,
                ]
                .spacing(4),
            ]
            .spacing(16),
        ]
        .spacing(8);

        let content = row![visualisers, controls].spacing(24).padding(20);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        iced::time::every(std::time::Duration::from_millis(33)).map(|_| Message::Tick)
    }
}

/// Two detuned saw oscillators mixed together, with atomic param control.
struct DetuneOsc<F: Signal, D: Signal> {
    phase1: f32,
    phase2: f32,
    freq: F,
    detune: D,
}

impl<F: Signal, D: Signal> Signal for DetuneOsc<F, D> {
    fn next(&mut self, ctx: &nyx_core::AudioContext) -> f32 {
        let freq = self.freq.next(ctx);
        let detune = self.detune.next(ctx);

        let saw1 = 2.0 * self.phase1 - 1.0;
        let saw2 = 2.0 * self.phase2 - 1.0;

        self.phase1 += freq / ctx.sample_rate;
        self.phase1 -= self.phase1.floor();
        self.phase2 += (freq * detune) / ctx.sample_rate;
        self.phase2 -= self.phase2.floor();

        (saw1 + saw2) * 0.5
    }
}
