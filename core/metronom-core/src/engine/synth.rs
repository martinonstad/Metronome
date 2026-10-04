//! Click sound synthesis. Sounds are generated in code so the app ships no audio assets.

use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accent {
    Strong,
    Normal,
}

/// Monophonic click voice: a short sine burst with a soft attack and exponential decay.
/// Triggering again simply restarts it.
pub struct Synth {
    sample_rate: f32,
    phase: f32,
    step: f32,
    age: u32,
    length: u32,
    gain: f32,
}

const ATTACK_SECONDS: f32 = 0.0005;
const DECAY_SECONDS: f32 = 0.008;
const LENGTH_SECONDS: f32 = 0.05;

impl Synth {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            phase: 0.0,
            step: 0.0,
            age: 0,
            length: 0,
            gain: 0.0,
        }
    }

    pub fn trigger(&mut self, accent: Accent) {
        let (hz, gain) = match accent {
            Accent::Strong => (1600.0, 1.0),
            Accent::Normal => (1000.0, 0.7),
        };
        self.phase = 0.0;
        self.step = TAU * hz / self.sample_rate;
        self.age = 0;
        self.length = (LENGTH_SECONDS * self.sample_rate) as u32;
        self.gain = gain;
    }

    /// Add the voice into `out`. Output depends only on how many frames the voice has played,
    /// never on how the stream is split into blocks.
    pub fn render(&mut self, out: &mut [f32]) {
        for sample in out {
            if self.age >= self.length {
                return;
            }
            let t = self.age as f32 / self.sample_rate;
            let attack = (t / ATTACK_SECONDS).min(1.0);
            let envelope = attack * (-t / DECAY_SECONDS).exp();
            *sample += self.phase.sin() * envelope * self.gain;
            self.phase = (self.phase + self.step) % TAU;
            self.age += 1;
        }
    }
}
