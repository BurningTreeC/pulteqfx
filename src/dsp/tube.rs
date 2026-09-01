//! The make-up amplifier.
//!
//! The passive EQ network throws away roughly 24 dB, which the EQP-1A's tube
//! amplifier puts back. That amplifier is the other half of the unit's
//! reputation: a push-pull triode stage into an output transformer, so it
//! contributes mostly odd order harmonics with a little second order from the
//! stage imbalance, plus the bandwidth limits of the iron at both ends.
//!
//! The `drive` control is the one liberty this plugin takes with the hardware.
//! At zero it is close to a straight wire; the original sits somewhere around
//! the lower quarter of the range at normal operating level.

/// A one pole filter used for the amplifier's band limits.
#[derive(Default)]
struct OnePole {
    a: f64,
    z: f64,
}

impl OnePole {
    fn set_cutoff(&mut self, freq: f64, sample_rate: f64) {
        let f = freq.clamp(1.0, 0.45 * sample_rate);
        // Matched one pole coefficient.
        self.a = (-std::f64::consts::TAU * f / sample_rate).exp();
    }

    #[inline]
    fn lowpass(&mut self, x: f64) -> f64 {
        self.z = x * (1.0 - self.a) + self.z * self.a;
        self.z
    }

    #[inline]
    fn highpass(&mut self, x: f64) -> f64 {
        x - self.lowpass(x)
    }

    fn reset(&mut self) {
        self.z = 0.0;
    }
}

pub struct TubeStage {
    /// Coupling transformer roll off.
    coupling: OnePole,
    /// Output transformer roll off.
    bandwidth: OnePole,
    /// Saturation hardness.
    k: f64,
    /// Operating point offset, the source of the second harmonic.
    bias: f64,
    /// Small signal gain of the shaper, divided back out.
    norm: f64,
}

impl TubeStage {
    pub fn new(sample_rate: f64) -> Self {
        let mut stage = Self {
            coupling: OnePole::default(),
            bandwidth: OnePole::default(),
            k: 0.4,
            bias: BIAS,
            norm: 1.0,
            
        };
        stage.set_sample_rate(sample_rate);
        stage.set_drive(0.0);
        stage
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.coupling.set_cutoff(COUPLING_HZ, sample_rate);
        self.bandwidth.set_cutoff(BANDWIDTH_HZ, sample_rate);
    }

    /// `drive` runs from 0 (nearly clean) to 1 (obviously coloured).
    pub fn set_drive(&mut self, drive: f64) {
        let drive = drive.clamp(0.0, 1.0);
        self.k = 0.4 + 5.6 * drive * drive;
        self.bias = BIAS * (0.3 + 0.7 * drive);
        // d/dx of the shaper at x = 0, so the small signal gain stays at unity
        // no matter where the drive control sits.
        let kb = (self.k * self.bias).tanh();
        self.norm = self.k * (1.0 - kb * kb);
    }

    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        let x = self.coupling.highpass(x);
        let kb = (self.k * self.bias).tanh();
        let y = ((self.k * (x + self.bias)).tanh() - kb) / self.norm;
        self.bandwidth.lowpass(y)
    }

    pub fn reset(&mut self) {
        self.coupling.reset();
        self.bandwidth.reset();
    }
}

/// Input coupling roll off, in Hz.
const COUPLING_HZ: f64 = 3.0;
/// Output bandwidth limit, in Hz.
const BANDWIDTH_HZ: f64 = 60e3;
/// Maximum operating point offset.
const BIAS: f64 = 0.12;
