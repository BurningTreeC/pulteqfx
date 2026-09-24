//! What the DRIVE control does to the amplifier.
//!
//! It raises the level into the stage, as hitting the hardware harder would,
//! so the sound gets louder and dirtier together and the output trim brings
//! the level back. Up to 0.11 it did something else -- a saturation amount
//! that held the level and squashed only the peaks -- and the built-in preset
//! that used it was moved across to settings that sound the same.

use pulteqfx::dsp::TubeStage;
use std::f64::consts::TAU;

const FS: f64 = 96_000.0;
/// A whole number of samples per cycle, so a transform over whole cycles has
/// nothing leaking between its bins.
const FREQ: f64 = 1_000.0;
const PERIOD: usize = 96;

/// The level of the fundamental relative to the input, and of the second and
/// third harmonics relative to the fundamental, all in dB, for a sine at
/// `level_db` dBFS through `curve`.
fn harmonics(level_db: f64, mut curve: impl FnMut(f64) -> f64) -> (f64, f64, f64) {
    let amplitude = 10f64.powf(level_db / 20.0);
    let x = |n: usize| amplitude * (TAU * FREQ * n as f64 / FS).sin();
    // Long enough for the 3 Hz coupling to settle.
    let settle = PERIOD * 500;
    for n in 0..settle {
        curve(x(n));
    }
    let span = PERIOD * 50;
    let out: Vec<f64> = (settle..settle + span).map(|n| curve(x(n))).collect();
    let bin = |k: f64| {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, y) in out.iter().enumerate() {
            let phase = TAU * k * FREQ * (settle + i) as f64 / FS;
            re += y * phase.cos();
            im += y * phase.sin();
        }
        2.0 * re.hypot(im) / span as f64
    };
    let (h1, h2, h3) = (bin(1.0), bin(2.0), bin(3.0));
    let db = |v: f64| 20.0 * v.log10();
    (db(h1 / amplitude), db(h2 / h1), db(h3 / h1))
}

fn stage(drive_db: f64, output_db: f64) -> impl FnMut(f64) -> f64 {
    let mut tube = TubeStage::new(FS);
    tube.set_drive(drive_db);
    let trim = 10f64.powf(output_db / 20.0);
    move |x| tube.process(x) * trim
}

/// Below where it bends, the stage passes a signal up by exactly the drive.
#[test]
fn drive_raises_the_level_by_what_it_says() {
    for drive in [0.0, 6.0, 12.0, 18.0] {
        let (level, _, _) = harmonics(-60.0, stage(drive, 0.0));
        assert!(
            (level - drive).abs() < 0.02,
            "{drive} dB of drive raised a quiet signal by {level:.3} dB"
        );
    }
}

/// And the harder it is hit, the dirtier it gets: clean at the usual operating
/// level with no drive, and plainly driven with all of it.
#[test]
fn the_harder_it_is_hit_the_dirtier_it_gets() {
    let mut last = f64::NEG_INFINITY;
    for drive in [0.0, 6.0, 12.0, 18.0] {
        let (_, h2, h3) = harmonics(-18.0, stage(drive, 0.0));
        let total = 10.0 * (10f64.powf(h2 / 10.0) + 10f64.powf(h3 / 10.0)).log10();
        println!("drive {drive:>4} dB: H2 {h2:6.1} dB, H3 {h3:6.1} dB");
        assert!(total > last, "{drive} dB of drive is no dirtier than less");
        last = total;
        match drive as i32 {
            0 => assert!(total < -55.0, "not clean with no drive: {total:.1} dB"),
            18 => assert!(total > -40.0, "not driven with all of it: {total:.1} dB"),
            _ => {}
        }
    }
}

/// Low End Punch, moved across from the old drive to the new one, sounds as it
/// did: the old curve is kept here as the reference it was moved from.
#[test]
fn low_end_punch_sounds_as_it_did_before_the_drive_changed() {
    let dials = pulteqfx::presets::built_in_dials("Low End Punch").expect("the preset");
    let dial = |id: &str| {
        dials
            .iter()
            .find(|(name, _)| *name == id)
            .map(|(_, value)| *value as f64)
            .unwrap_or_else(|| panic!("Low End Punch has no {id}"))
    };
    let (drive, output) = (dial("drivedb"), dial("output"));

    // The amplifier up to 0.11, at the preset's 25 %. It is a curve only; the
    // coupling and the band limit, which did not change, are left out, and at
    // 1 kHz they make no difference worth measuring.
    let old = |x: f64| {
        let d = 0.25;
        let k = 0.4 + 5.6 * d * d;
        let b = 0.12 * (0.3 + 0.7 * d);
        let rest = f64::tanh(k * b);
        (f64::tanh(k * (x + b)) - rest) / (k * (1.0 - rest * rest))
    };

    for level in [-24.0, -18.0, -12.0, -6.0, 0.0, 3.0] {
        let (was, was_h2, was_h3) = harmonics(level, old);
        let (now, now_h2, now_h3) = harmonics(level, stage(drive, output));
        println!(
            "{level:+5} dBFS: level {was:+.3} -> {now:+.3}, H2 {was_h2:.1} -> {now_h2:.1}, H3 {was_h3:.1} -> {now_h3:.1}"
        );
        assert!((now - was).abs() < 0.02, "the level moved at {level} dBFS");
        assert!(
            (now_h3 - was_h3).abs() < 0.2,
            "the third harmonic moved at {level} dBFS"
        );
        assert!(
            (now_h2 - was_h2).abs() < 1.5,
            "the second harmonic moved at {level} dBFS"
        );
    }
}
