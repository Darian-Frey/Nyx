//! Bevy spectrum visualiser — FFT bars rendered as ECS entities.
//!
//! Plays a chord pad and renders the spectrum as coloured bars that
//! rise and fall with the magnitude of each frequency band.
//!
//! Run: cargo run -p nyx-examples --example bevy_spectrum --release

use bevy::prelude::*;
use nyx_prelude::*;

const BAR_COUNT: usize = 64;
const WINDOW_W: f32 = 900.0;
const WINDOW_H: f32 = 400.0;

/// Resource wrapping the audio-side handle that the render thread polls.
#[derive(Resource)]
struct Audio {
    _engine: Engine,
    spectrum: SpectrumHandle,
}

/// Marker + index for each bar so we can update its height by position.
#[derive(Component)]
struct Bar(usize);

fn main() {
    // Exponential frequency sweep from 40 Hz to 16 kHz, cycling every 8s.
    // Visually steps through every bar so you can verify the full range.
    let sweep = automation::automation(|t| {
        let phase = (t / 8.0).fract(); // 0..1 over 8 seconds
        40.0 * 400.0_f32.powf(phase)   // 40 Hz → 40 * 400 = 16000 Hz
    });
    let sig = osc::sine(sweep).amp(0.3);

    let (sig, spectrum) = sig.spectrum(SpectrumConfig {
        frame_size: 2048,
        window: WindowFn::Hann,
    });

    let engine = Engine::play(sig).expect("failed to open audio device");

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Nyx Spectrum".into(),
                resolution: (WINDOW_W, WINDOW_H).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.08, 0.08, 0.10)))
        .insert_resource(Audio {
            _engine: engine,
            spectrum,
        })
        .add_systems(Startup, spawn_bars)
        .add_systems(Update, update_bars)
        .run();
}

fn spawn_bars(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());

    let bar_w = WINDOW_W / BAR_COUNT as f32;
    for i in 0..BAR_COUNT {
        let x = -WINDOW_W / 2.0 + bar_w * (i as f32 + 0.5);
        // Colour interpolation: blue at low freqs → pink at high freqs.
        let t = i as f32 / BAR_COUNT as f32;
        let color = Color::srgb(
            0.0 + t * 1.0,
            0.4 - t * 0.1,
            0.8 - t * 0.3,
        );
        commands.spawn((
            SpriteBundle {
                sprite: Sprite {
                    color,
                    custom_size: Some(Vec2::new(bar_w - 2.0, 1.0)),
                    ..default()
                },
                transform: Transform::from_xyz(x, -WINDOW_H / 2.0, 0.0),
                ..default()
            },
            Bar(i),
        ));
    }
}

fn update_bars(audio: Res<Audio>, mut query: Query<(&Bar, &mut Transform, &mut Sprite)>) {
    let bins = audio.spectrum.snapshot();
    if bins.is_empty() {
        return;
    }

    let max_mag = bins.iter().map(|b| b.magnitude).fold(0.0_f32, f32::max).max(1e-10);
    let total_bins = bins.len();

    for (bar, mut xf, mut sprite) in &mut query {
        // Split bins into BAR_COUNT groups, distributing any remainder
        // evenly rather than dropping it. This ensures the highest
        // frequencies are always represented even when bin count isn't
        // divisible by BAR_COUNT.
        let start = (bar.0 * total_bins) / BAR_COUNT;
        let end = ((bar.0 + 1) * total_bins) / BAR_COUNT;
        let span = end.saturating_sub(start).max(1);
        let avg: f32 = bins[start..end.max(start + 1).min(total_bins)]
            .iter()
            .map(|b| b.magnitude)
            .sum::<f32>()
            / span as f32;
        let normalized = (avg / max_mag).clamp(0.0, 1.0);

        let height = normalized * (WINDOW_H * 0.9);
        if let Some(size) = sprite.custom_size.as_mut() {
            size.y = height.max(1.0);
        }
        xf.translation.y = -WINDOW_H / 2.0 + height / 2.0;
    }
}
