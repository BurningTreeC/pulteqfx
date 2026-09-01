//! Signal processing for PultEQFx.

pub mod eqp1a;
pub mod nodal;
pub mod oversample;
pub mod tube;


pub use eqp1a::{Controls, Eqp1a, HIGH_ATTEN_FREQS, HIGH_BOOST_FREQS, LOW_FREQS};
pub use oversample::Oversampler;
pub use tube::TubeStage;

/// Latency the plugin reports, in samples at the host rate. It is the round
/// trip through the longest oversampling cascade; shorter settings are padded
/// out to match so the reported latency never changes while the plugin is
/// running, which the CLAP specification asks for and hosts are happier with.
pub const LATENCY: u32 = 74;

/// Delays the signal to make up whatever the oversampler is not using.
struct Padding {
    buf: [f32; LATENCY as usize],
    pos: usize,
    len: usize,
}

impl Padding {
    fn new() -> Self {
        Self {
            buf: [0.0; LATENCY as usize],
            pos: 0,
            len: 0,
        }
    }

    fn set_len(&mut self, len: usize) {
        self.len = len.min(self.buf.len());
        self.buf.fill(0.0);
        self.pos = 0;
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        if self.len == 0 {
            return x;
        }
        let out = self.buf[self.pos];
        self.buf[self.pos] = x;
        self.pos += 1;
        if self.pos == self.len {
            self.pos = 0;
        }
        out
    }

    fn reset(&mut self) {
        self.buf.fill(0.0);
        self.pos = 0;
    }
}

/// One channel of the unit: the passive network, the make-up amplifier, and
/// the oversampling that surrounds both.
pub struct Channel {
    pub eq: Eqp1a,
    pub tube: TubeStage,
    oversampler: Oversampler,
    padding: Padding,
    sample_rate: f64,
}

impl Channel {
    pub fn new(sample_rate: f64, factor: usize) -> Self {
        let oversampler = Oversampler::new(factor);
        let internal = sample_rate * oversampler.factor() as f64;
        let mut padding = Padding::new();
        padding.set_len((LATENCY - oversampler.latency()) as usize);
        Self {
            eq: Eqp1a::new(internal),
            tube: TubeStage::new(internal),
            oversampler,
            padding,
            sample_rate,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
        let internal = sample_rate * self.oversampler.factor() as f64;
        self.eq.set_sample_rate(internal);
        self.tube.set_sample_rate(internal);
        self.reset();
    }

    /// Allocation free: every stage already exists, this only changes how many
    /// of them are in use.
    pub fn set_oversampling(&mut self, factor: usize) {
        if factor == self.oversampler.factor() {
            return;
        }
        self.oversampler.set_factor(factor);
        self.padding
            .set_len((LATENCY - self.oversampler.latency()) as usize);
        let internal = self.sample_rate * self.oversampler.factor() as f64;
        self.eq.set_sample_rate(internal);
        self.tube.set_sample_rate(internal);
        self.reset();
    }

    /// Always the same, whatever the oversampling setting is.
    pub fn latency(&self) -> u32 {
        LATENCY
    }

    /// `eq_in` mirrors the front panel EQ IN/OUT switch, which lifts the
    /// passive network out of circuit but leaves the amplifier in it.
    #[inline]
    pub fn process(&mut self, sample: f32, eq_in: bool) -> f32 {
        let Self {
            eq,
            tube,
            oversampler,
            padding,
            ..
        } = self;
        let out = oversampler.process(sample as f64, &mut |x| {
            let x = if eq_in { eq.process(x) } else { x };
            tube.process(x)
        });
        padding.process(out as f32)
    }

    pub fn reset(&mut self) {
        self.eq.reset();
        self.tube.reset();
        self.oversampler.reset();
        self.padding.reset();
    }
}
