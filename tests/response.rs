//! Frequency response checks against the EQP-1A's published behaviour.
//!
//! The response is taken from the impulse response of the passive network,
//! which is linear, so a direct transform at the frequencies of interest gives
//! the exact magnitude without needing a settling time.

use pulteqfx::dsp::{Channel, Controls, Eqp1a};

const FS: f64 = 96_000.0;
/// Long enough for the slowest low frequency shelf to die away completely.
const IR_LEN: usize = 1 << 18;

fn impulse_response(controls: Controls) -> Vec<f64> {
    let mut eq = Eqp1a::new(FS);
    eq.set_controls(controls);
    let mut ir = Vec::with_capacity(IR_LEN);
    ir.push(eq.process(1.0));
    for _ in 1..IR_LEN {
        ir.push(eq.process(0.0));
    }
    ir
}

/// Magnitude of the transform of `ir` at `freq`, in dB.
fn magnitude_db(ir: &[f64], freq: f64) -> f64 {
    let w = std::f64::consts::TAU * freq / FS;
    let (mut re, mut im) = (0.0, 0.0);
    for (n, &x) in ir.iter().enumerate() {
        let phase = w * n as f64;
        re += x * phase.cos();
        im -= x * phase.sin();
    }
    20.0 * (re * re + im * im).sqrt().log10()
}

const SWEEP: [f64; 18] = [
    20.0, 30.0, 50.0, 80.0, 100.0, 150.0, 200.0, 300.0, 500.0, 800.0, 1000.0, 2000.0, 3000.0,
    5000.0, 8000.0, 10000.0, 16000.0, 20000.0,
];

fn curve(label: &str, controls: Controls) -> Vec<f64> {
    let ir = impulse_response(controls);
    let db: Vec<f64> = SWEEP.iter().map(|&f| magnitude_db(&ir, f)).collect();
    println!("{label:>22}  {}", fmt(&db));
    db
}

fn fmt(db: &[f64]) -> String {
    db.iter()
        .map(|d| format!("{d:+6.1}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn header() {
    println!(
        "{:>22}  {}",
        "",
        SWEEP
            .iter()
            .map(|f| format!("{:>6.0}", f))
            .collect::<Vec<_>>()
            .join(" ")
    );
}

fn at(db: &[f64], freq: f64) -> f64 {
    db[SWEEP.iter().position(|&f| f == freq).unwrap()]
}

fn peak(db: &[f64]) -> f64 {
    db.iter().cloned().fold(f64::MIN, f64::max)
}

fn trough(db: &[f64]) -> f64 {
    db.iter().cloned().fold(f64::MAX, f64::min)
}

#[test]
fn flat_when_every_knob_is_down() {
    header();
    let db = curve("flat", Controls::default());
    for (f, d) in SWEEP.iter().zip(db.iter()) {
        assert!(d.abs() < 0.5, "expected unity at {f} Hz, got {d:+.2} dB");
    }
}

#[test]
fn low_boost_matches_the_published_curves() {
    header();
    // Measured on a real unit: roughly +16 dB at the bottom on the 100 cps
    // setting, tapering to about +11 dB on the 20 cps setting where the shelf
    // has not fully turned over by 20 Hz.
    let expected = [(20.0, 11.7), (30.0, 14.9), (60.0, 16.1), (100.0, 16.3)];
    for (freq, target) in expected {
        let db = curve(
            &format!("low boost 10 @ {freq}"),
            Controls {
                low_boost: 1.0,
                low_freq: freq,
                ..Controls::default()
            },
        );
        let got = at(&db, 20.0);
        assert!(
            (got - target).abs() < 1.0,
            "low boost at {freq} Hz: expected about {target:+.1} dB at 20 Hz, got {got:+.2}"
        );
        // The shelf must be over by the midrange.
        assert!(at(&db, 1000.0).abs() < 2.0);
        assert!(at(&db, 3000.0).abs() < 0.5);
    }
}

#[test]
fn low_atten_matches_the_published_curves() {
    header();
    let expected = [(20.0, -15.1), (30.0, -17.8), (60.0, -18.9), (100.0, -19.1)];
    for (freq, target) in expected {
        let db = curve(
            &format!("low atten 10 @ {freq}"),
            Controls {
                low_atten: 1.0,
                low_freq: freq,
                ..Controls::default()
            },
        );
        let got = at(&db, 20.0);
        assert!(
            (got - target).abs() < 1.0,
            "low atten at {freq} Hz: expected about {target:+.1} dB at 20 Hz, got {got:+.2}"
        );
    }
}

#[test]
fn high_boost_peaks_at_eighteen_decibels_on_the_selected_frequency() {
    header();
    for freq in [3e3, 5e3, 8e3, 10e3, 16e3] {
        let sharp = curve(
            &format!("hi boost 10 sharp @{freq}"),
            Controls {
                high_boost: 1.0,
                high_boost_freq: freq,
                bandwidth: 0.0,
                ..Controls::default()
            },
        );
        let broad = curve(
            &format!("hi boost 10 broad @{freq}"),
            Controls {
                high_boost: 1.0,
                high_boost_freq: freq,
                bandwidth: 1.0,
                ..Controls::default()
            },
        );
        let peak_sharp = peak(&sharp);
        assert!(
            (peak_sharp - 18.0).abs() < 1.0,
            "sharp boost at {freq} Hz peaked at {peak_sharp:+.2} dB, expected +18"
        );
        // Broadening the bandwidth trades height for width.
        assert!(
            peak(&broad) < peak_sharp - 3.0,
            "broad should be lower than sharp at {freq} Hz"
        );
        // The bell must sit on the selected frequency.
        let idx_at_freq = SWEEP.iter().position(|&f| (f - freq).abs() < 1.0).unwrap();
        let best = sharp
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        assert_eq!(
            best, idx_at_freq,
            "bell for {freq} Hz peaked at {} Hz instead",
            SWEEP[best]
        );
    }
}

#[test]
fn high_atten_reaches_sixteen_decibels_on_the_selected_frequency() {
    header();
    for freq in [5e3, 10e3, 20e3] {
        let db = curve(
            &format!("hi atten 10 @{freq}"),
            Controls {
                high_atten: 1.0,
                high_atten_freq: freq,
                ..Controls::default()
            },
        );
        let idx = SWEEP.iter().position(|&f| (f - freq).abs() < 1.0).unwrap();
        assert!(
            (db[idx] + 16.0).abs() < 1.2,
            "atten at {freq} Hz reached {:+.2} dB, expected about -16",
            db[idx]
        );
        // A shelf, so it must keep going below that.
        assert!(trough(&db) <= db[idx] + 0.01);
    }
}

#[test]
fn the_low_end_trick_gives_a_bump_and_a_dip() {
    header();
    // The classic description: boosting and attenuating 30 cps together lifts
    // around 80 Hz and dips around 200 Hz.
    let db = curve(
        "trick @30",
        Controls {
            low_boost: 1.0,
            low_atten: 1.0,
            low_freq: 30.0,
            ..Controls::default()
        },
    );
    assert!(
        at(&db, 80.0) > 3.0,
        "expected a lift around 80 Hz, got {:+.2} dB",
        at(&db, 80.0)
    );
    assert!(
        at(&db, 200.0) < -2.5,
        "expected a dip around 200 Hz, got {:+.2} dB",
        at(&db, 200.0)
    );
    assert!(at(&db, 1000.0).abs() < 1.0, "the trick must be over by 1 kHz");

    // The same shape moves up with the frequency selector.
    let db = curve(
        "trick @100",
        Controls {
            low_boost: 1.0,
            low_atten: 1.0,
            low_freq: 100.0,
            ..Controls::default()
        },
    );
    assert!(at(&db, 100.0) > 8.0);
    let dip = SWEEP
        .iter()
        .zip(db.iter())
        .filter(|(f, _)| (300.0..=1500.0).contains(*f))
        .map(|(_, d)| *d)
        .fold(f64::MAX, f64::min);
    assert!(dip < -3.0, "expected a dip above the bump, got {dip:+.2} dB");
}

#[test]
fn boosting_and_attenuating_the_highs_together_shapes_the_top() {
    header();
    let db = curve(
        "hi boost + atten @10k",
        Controls {
            high_boost: 1.0,
            high_boost_freq: 10e3,
            high_atten: 1.0,
            high_atten_freq: 10e3,
            bandwidth: 0.5,
            ..Controls::default()
        },
    );
    // A gentle scoop below, a peak on the selected frequency.
    assert!(at(&db, 1000.0) < 0.0);
    assert!(at(&db, 10000.0) > 8.0);
}

#[test]
fn the_full_chain_is_transparent_with_the_eq_out() {
    // Whole plugin path: oversampling, the amplifier, the lot.
    let mut channel = Channel::new(FS, 4);
    channel.tube.set_drive(0.0);
    let mut ir = Vec::with_capacity(IR_LEN);
    ir.push(channel.process(1e-3, false) as f64 * 1e3);
    for _ in 1..IR_LEN {
        ir.push(channel.process(0.0, false) as f64 * 1e3);
    }
    header();
    let db: Vec<f64> = SWEEP.iter().map(|&f| magnitude_db(&ir, f)).collect();
    println!("{:>22}  {}", "eq out", fmt(&db));
    for (f, d) in SWEEP.iter().zip(db.iter()) {
        assert!(
            d.abs() < 0.6,
            "amplifier path should be near flat at {f} Hz, got {d:+.2} dB"
        );
    }
}

/// The built-in preset should do what its name says: weight at the bottom, a
/// scoop through the low mids, and air on top.
#[test]
fn low_end_punch_preset_has_punch() {
    let dials = pulteqfx::presets::built_in_dials("Low End Punch")
        .expect("the Low End Punch preset should exist");

    let dial = |id: &str| {
        dials
            .iter()
            .find(|(name, _)| *name == id)
            .map(|(_, value)| *value as f64)
            .unwrap_or_else(|| panic!("preset is missing {id}"))
    };

    let controls = Controls {
        low_boost: dial("loboost") / 10.0,
        low_atten: dial("loatten") / 10.0,
        high_boost: dial("hiboost") / 10.0,
        high_atten: dial("hiatten") / 10.0,
        bandwidth: dial("bandw") / 10.0,
        low_freq: [20.0, 30.0, 60.0, 100.0][dial("lofreq") as usize],
        high_boost_freq: [3e3, 4e3, 5e3, 8e3, 10e3, 12e3, 16e3][dial("hifreq") as usize],
        high_atten_freq: [5e3, 10e3, 20e3][dial("hiafreq") as usize],
    };

    header();
    let db = curve("low end punch", controls);

    assert!(
        at(&db, 50.0) > 5.0,
        "expected weight at the bottom, got {:+.2} dB at 50 Hz",
        at(&db, 50.0)
    );
    assert!(
        at(&db, 100.0) > 4.0,
        "expected punch at 100 Hz, got {:+.2} dB",
        at(&db, 100.0)
    );
    // The scoop the low end trick opens up between the bump and the midrange.
    let scoop = SWEEP
        .iter()
        .zip(db.iter())
        .filter(|(f, _)| (300.0..=800.0).contains(*f))
        .map(|(_, d)| *d)
        .fold(f64::MAX, f64::min);
    assert!(
        scoop < 0.5,
        "expected a scoop through the low mids, the lowest point was {scoop:+.2} dB"
    );
    assert!(
        at(&db, 10000.0) > 3.0,
        "expected air on top, got {:+.2} dB at 10 kHz",
        at(&db, 10000.0)
    );
}
