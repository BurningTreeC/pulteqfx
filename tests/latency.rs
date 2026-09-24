//! The latency the plugin reports has to stay true whatever the panel does.
//!
//! The host is told `LATENCY` once, at initialisation, and delays every other
//! track to match. Anything the plugin then lets through sooner or later than
//! that is out of time with the rest of the session. The power switch used to
//! be exactly that: off passed the input straight through, 74 samples ahead
//! of everything else, and throwing it moved the track back and forth.

use pulteqfx::dsp::{Channel, Controls, CROSSFADE, LATENCY, WARM_UP};
use std::f64::consts::TAU;

const FS: f64 = 48_000.0;
const FACTORS: [usize; 4] = [1, 2, 4, 8];

/// Something with more in it than one sine, so that a path answering with the
/// wrong samples cannot match by coincidence.
fn input(n: usize) -> f32 {
    let t = n as f64 / FS;
    (0.3 * (TAU * 110.0 * t).sin() + 0.1 * (TAU * 3_700.0 * t).sin()) as f32
}

/// With the power off the input comes back untouched, but exactly as late as
/// the host has been told.
#[test]
fn power_off_holds_the_dry_signal_back_by_the_reported_latency() {
    for factor in FACTORS {
        let mut channel = Channel::new(FS, factor);
        for n in 0..512 {
            let got = channel.process(if n == 0 { 1.0 } else { 0.0 }, true, false);
            let expected = if n == LATENCY as usize { 1.0 } else { 0.0 };
            assert_eq!(got, expected, "sample {n} at {factor}x oversampling");
        }
    }
}

/// The processed path arrives on the same sample, at every oversampling
/// setting, or throwing the switch would still move the track.
#[test]
fn the_processed_path_arrives_when_the_dry_one_does() {
    for factor in FACTORS {
        let mut channel = Channel::new(FS, factor);
        channel.set_drive(0.0);
        let response: Vec<f32> = (0..512)
            .map(|n| channel.process(if n == 0 { 1e-3 } else { 0.0 }, false, true))
            .collect();
        let peak = response
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .map(|(n, _)| n)
            .unwrap();
        assert_eq!(
            peak, LATENCY as usize,
            "at {factor}x oversampling the processed path peaks at sample {peak}"
        );
    }
}

/// Throwing the switch mid-stream picks up whichever path it lands on exactly
/// where that path would have been had it been selected all along: the dry
/// delay was filled while the power was on, and the processed path kept
/// running while it was off. Starting either over leaves a gap as long as the
/// latency.
#[test]
fn throwing_the_power_switch_picks_each_path_up_mid_stream() {
    for factor in FACTORS {
        let mut on = Channel::new(FS, factor);
        let mut off = Channel::new(FS, factor);
        let mut switched = Channel::new(FS, factor);
        for n in 0..8_000 {
            let x = input(n);
            let lit = on.process(x, true, true);
            let dark = off.process(x, true, false);
            // Off and on in stretches shorter and longer than the latency.
            let powered = matches!(n % 1_000, 0..=39 | 300..=599);
            let got = switched.process(x, true, powered);
            let expected = if powered { lit } else { dark };
            assert_eq!(
                got,
                expected,
                "sample {n} at {factor}x oversampling, power {}",
                if powered { "on" } else { "off" }
            );
        }
    }
}

/// Changing the oversampling restarts the processed path, but with the power
/// off that path is not what is coming out, so nothing may happen to it.
#[test]
fn changing_the_oversampling_with_the_power_off_leaves_the_signal_alone() {
    let mut steady = Channel::new(FS, 4);
    let mut changed = Channel::new(FS, 4);
    for n in 0..4_000 {
        if n % 1_000 == 500 {
            changed.set_oversampling(FACTORS[(n / 1_000) % FACTORS.len()]);
        }
        let x = input(n);
        assert_eq!(
            changed.process(x, true, false),
            steady.process(x, true, false),
            "sample {n}"
        );
    }
}

/// A setting of the knobs with long memories in it, so that a path starting
/// over from rest would take a while to catch up with one that had not.
fn busy() -> Controls {
    Controls {
        low_boost: 0.8,
        low_atten: 0.5,
        low_freq: 30.0,
        high_boost: 0.5,
        high_boost_freq: 5e3,
        ..Controls::default()
    }
}

/// Changing the oversampling while playing used to start the path over: the
/// length of the latency in silence, then the network settling from rest.
/// Now the old setting goes on being heard, untouched, while the new one
/// warms up unheard, and then the one fades into the other.
#[test]
fn changing_the_oversampling_while_playing_leaves_no_gap() {
    let at = 3_000;
    let warm_up = (WARM_UP * FS).round() as usize;
    let fade = (CROSSFADE * FS).round() as usize;
    for (from, to) in [(1, 4), (4, 1), (2, 8), (8, 2)] {
        let mut before = Channel::new(FS, from);
        let mut after = Channel::new(FS, to);
        let mut changed = Channel::new(FS, from);
        for channel in [&mut before, &mut after, &mut changed] {
            channel.set_controls(busy());
            channel.set_drive(0.25);
        }

        let (mut strayed, mut behind) = (0.0f32, 0.0f32);
        for n in 0..at + warm_up + fade + 4_000 {
            if n == at {
                changed.set_oversampling(to);
            }
            let x = input(n);
            let old = before.process(x, true, true);
            let new = after.process(x, true, true);
            let got = changed.process(x, true, true);
            if n < at + warm_up {
                assert_eq!(
                    got, old,
                    "{from}x to {to}x: sample {n} is not the old setting"
                );
            } else if n < at + warm_up + fade {
                // Somewhere between the two, never off towards silence.
                strayed = strayed.max((old.min(new) - got).max(got - old.max(new)));
            } else {
                behind = behind.max((got - new).abs());
            }
        }
        println!("{from}x to {to}x: strayed {strayed:.2e} during the fade, {behind:.2e} after it");
        assert!(
            strayed < 1e-3,
            "{from}x to {to}x: the fade left the two settings by {strayed}"
        );
        assert!(
            behind < 1e-3,
            "{from}x to {to}x: the new setting is {behind} off where it would have been"
        );
    }
}

/// Changing it back before the new setting was ever heard is simply staying
/// where it was.
#[test]
fn changing_the_oversampling_back_at_once_changes_nothing() {
    let mut steady = Channel::new(FS, 4);
    let mut changed = Channel::new(FS, 4);
    for channel in [&mut steady, &mut changed] {
        channel.set_controls(busy());
    }
    for n in 0..20_000 {
        match n {
            1_000 => changed.set_oversampling(8),
            1_500 => changed.set_oversampling(2),
            2_000 => changed.set_oversampling(4),
            _ => {}
        }
        let x = input(n);
        assert_eq!(
            changed.process(x, true, true),
            steady.process(x, true, true),
            "sample {n}"
        );
    }
}
