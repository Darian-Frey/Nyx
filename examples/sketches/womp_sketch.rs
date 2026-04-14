use nyx_prelude::*;

#[unsafe(no_mangle)]
pub fn nyx_sketch() -> Box<dyn Signal> {
    // 1. Clock (140 BPM — dubstep/grime feel)
    let mut clk = clock::clock(140.0);

    // 2. The Wobble LFO — speed changes with time (build-up into "the drop")
    let wobble_speed = automation::automation(|t| {
        if t < 15.0 { 2.0 }      // slow wobble during intro
        else if t < 16.0 { 8.0 } // fast drill into the drop
        else { 4.0 }             // heavy wobble after the drop
    });
    let lfo = osc::sine(wobble_speed).amp(1500.0).offset(1600.0);

    // 3. Bass: detuned saws → resonant lowpass (wobble) → soft clip (grit)
    //    Highpass cutoff also automates: high at 300Hz during intro
    //    (thin, tinny), then drops to 20Hz for the drop (full bass).
    let hp_cutoff = automation::automation(|t| {
        if t < 16.0 { 300.0 } else { 20.0 }
    });
    let bass = osc::saw(55.0)
        .add(osc::saw(55.5).amp(0.5))
        .lowpass(lfo, 3.0)
        .highpass(hp_cutoff, 0.7)
        .soft_clip(2.0);

    // 4. Drums
    let mut kick = inst::kick();
    let mut snare = inst::snare();

    // Patterns — kick on 1, snare on 3 (in a bar of 8 sixteenths)
    let kick_pat = Pattern::new(&[true, false, false, false, false, false, false, false]);
    let snare_pat = Pattern::new(&[false, false, false, false, true, false, false, false]);
    let mut kick_seq = Sequence::new(kick_pat, 0.25);
    let mut snare_seq = Sequence::new(snare_pat, 0.25);

    // 5. Signal graph — drums kick in at t=16 (the drop)
    let mut bass = bass;
    let final_signal = move |ctx: &AudioContext| {
        let state = clk.tick(ctx);
        let time = ctx.tick as f32 / ctx.sample_rate;

        // Fire drum triggers on step boundaries
        let k_event = kick_seq.tick(&state);
        let s_event = snare_seq.tick(&state);
        if k_event.triggered && k_event.value { kick.trigger(); }
        if s_event.triggered && s_event.value { snare.trigger(); }

        // Sample all sources every tick (to keep state consistent)
        let bass_sample = bass.next(ctx);
        let kick_sample = kick.next(ctx);
        let snare_sample = snare.next(ctx);

        // Before the drop: bass only. After: full mix.
        let (audio_out, master_gain) = if time < 16.0 {
            (bass_sample, 0.6)
        } else {
            (bass_sample + kick_sample * 1.2 + snare_sample, 0.8)
        };

        audio_out * master_gain
    };

    final_signal.boxed()
}
