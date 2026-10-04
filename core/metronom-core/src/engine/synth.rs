//! Click sound synthesis. Sounds are generated in code so the app ships no audio assets.

use std::f32::consts::TAU;

use super::Sound;

/// How loud a click is, which also picks its pitch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strength {
    /// The first beat of the bar.
    Strong,
    /// Every other beat.
    Normal,
}

impl Strength {
    fn slot(self) -> usize {
        match self {
            Self::Strong => 0,
            Self::Normal => 1,
        }
    }
}

/// The recipe for one sound: a sine burst with an envelope, an optional downward pitch sweep and
/// an optional noise component. Arrays are indexed by [`Strength`] (strong, normal).
struct Profile {
    hz: [f32; 2],
    gain: [f32; 2],
    attack: f32,
    decay: f32,
    length: f32,
    /// Extra pitch at the start as a fraction of the base pitch, falling away over `sweep_tau`.
    sweep: f32,
    sweep_tau: f32,
    /// Share of noise in the mix, 0.0–1.0.
    noise: f32,
}

const CLICK: Profile = Profile {
    hz: [1600.0, 1000.0],
    gain: [1.0, 0.7],
    attack: 0.0005,
    decay: 0.008,
    length: 0.05,
    sweep: 0.0,
    sweep_tau: 0.001,
    noise: 0.0,
};

const WOOD: Profile = Profile {
    hz: [950.0, 760.0],
    gain: [1.0, 0.75],
    attack: 0.0003,
    decay: 0.014,
    length: 0.08,
    sweep: 0.6,
    sweep_tau: 0.004,
    noise: 0.05,
};

const BEEP: Profile = Profile {
    hz: [1760.0, 1320.0],
    gain: [0.8, 0.6],
    attack: 0.002,
    decay: 0.03,
    length: 0.09,
    sweep: 0.0,
    sweep_tau: 0.001,
    noise: 0.0,
};

const RIM: Profile = Profile {
    hz: [2600.0, 2100.0],
    gain: [0.9, 0.65],
    attack: 0.0002,
    decay: 0.005,
    length: 0.04,
    sweep: 0.0,
    sweep_tau: 0.001,
    noise: 0.35,
};

fn profile(sound: Sound) -> &'static Profile {
    match sound {
        Sound::Click => &CLICK,
        Sound::Wood => &WOOD,
        Sound::Beep => &BEEP,
        Sound::Rim => &RIM,
    }
}

/// The last moment of a click fades out linearly so it never ends with a tick.
const FADE_OUT_SECONDS: f32 = 0.002;
const NOISE_SEED: u32 = 0x9E37_79B9;

/// Monophonic click voice. Triggering again simply restarts it.
pub struct Synth {
    sample_rate: f32,
    profile: &'static Profile,
    base_hz: f32,
    gain: f32,
    phase: f32,
    age: u32,
    length: u32,
    noise_state: u32,
}

impl Synth {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            profile: &CLICK,
            base_hz: 0.0,
            gain: 0.0,
            phase: 0.0,
            age: 0,
            length: 0,
            noise_state: NOISE_SEED,
        }
    }

    pub fn trigger(&mut self, sound: Sound, strength: Strength) {
        let profile = profile(sound);
        let slot = strength.slot();
        self.profile = profile;
        self.base_hz = profile.hz[slot];
        self.gain = profile.gain[slot];
        self.phase = 0.0;
        self.age = 0;
        self.length = (profile.length * self.sample_rate) as u32;
        self.noise_state = NOISE_SEED;
    }

    /// Add the voice into `out`. Output depends only on how many frames the voice has played,
    /// never on how the stream is split into blocks.
    pub fn render(&mut self, out: &mut [f32]) {
        let p = self.profile;
        let fade_frames = FADE_OUT_SECONDS * self.sample_rate;
        for sample in out {
            if self.age >= self.length {
                return;
            }
            let t = self.age as f32 / self.sample_rate;
            let attack = (t / p.attack).min(1.0);
            let fade = ((self.length - self.age) as f32 / fade_frames).min(1.0);
            let envelope = attack * (-t / p.decay).exp() * fade;

            let tone = self.phase.sin();
            let mixed = if p.noise > 0.0 {
                self.noise_state = self
                    .noise_state
                    .wrapping_mul(1_664_525)
                    .wrapping_add(1_013_904_223);
                let noise = (self.noise_state >> 8) as f32 / 8_388_608.0 - 1.0;
                tone * (1.0 - p.noise) + noise * p.noise
            } else {
                tone
            };
            *sample += mixed * envelope * self.gain;

            let hz = self.base_hz * (1.0 + p.sweep * (-t / p.sweep_tau).exp());
            self.phase = (self.phase + TAU * hz / self.sample_rate) % TAU;
            self.age += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn render(sound: Sound, strength: Strength, frames: usize) -> Vec<f32> {
        let mut synth = Synth::new(SR);
        synth.trigger(sound, strength);
        let mut out = vec![0.0; frames];
        synth.render(&mut out);
        out
    }

    fn peak(samples: &[f32]) -> f32 {
        samples.iter().fold(0.0, |m, s| m.max(s.abs()))
    }

    #[test]
    fn every_sound_is_audible_bounded_and_ends_silent() {
        for sound in Sound::ALL {
            for strength in [Strength::Strong, Strength::Normal] {
                let out = render(sound, strength, 6_000);
                assert!(peak(&out) > 0.05, "{sound:?}/{strength:?} is too quiet");
                assert!(peak(&out) <= 1.0, "{sound:?}/{strength:?} clips");
                assert!(out.iter().all(|s| s.is_finite()));
                // After the voice's length (<= 90 ms) nothing more is written, and the last
                // written samples have faded out instead of ending with a tick.
                assert_eq!(
                    peak(&out[4_500..]),
                    0.0,
                    "{sound:?} still sounding at 94 ms"
                );
                let tail = out.iter().rposition(|&s| s != 0.0).unwrap_or(0);
                assert!(
                    out[tail].abs() < 2e-3,
                    "{sound:?}/{strength:?} ends with a tick"
                );
            }
        }
    }

    #[test]
    fn strong_is_louder_than_normal() {
        for sound in Sound::ALL {
            let strong = peak(&render(sound, Strength::Strong, 4_000));
            let normal = peak(&render(sound, Strength::Normal, 4_000));
            assert!(strong > normal, "{sound:?}: {strong} vs {normal}");
        }
    }

    #[test]
    fn the_sounds_are_audibly_different() {
        let sounds: Vec<Vec<f32>> = Sound::ALL
            .iter()
            .map(|&s| render(s, Strength::Normal, 4_000))
            .collect();
        for (i, a) in sounds.iter().enumerate() {
            for b in &sounds[i + 1..] {
                let difference: f32 = a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum();
                assert!(
                    difference > 5.0,
                    "two sounds are nearly identical ({difference})"
                );
            }
        }
    }

    #[test]
    fn a_click_is_identical_every_time_and_whatever_the_block_size() {
        for sound in Sound::ALL {
            let reference = render(sound, Strength::Strong, 4_000);
            assert_eq!(render(sound, Strength::Strong, 4_000), reference);
            for block in [1, 17, 480, 1024] {
                let mut synth = Synth::new(SR);
                synth.trigger(sound, Strength::Strong);
                let mut out = vec![0.0; 4_000];
                for chunk in out.chunks_mut(block) {
                    synth.render(chunk);
                }
                assert_eq!(out, reference, "{sound:?}, block size {block}");
            }
        }
    }

    #[test]
    fn retriggering_restarts_the_voice() {
        let mut synth = Synth::new(SR);
        synth.trigger(Sound::Click, Strength::Strong);
        let mut first = vec![0.0; 1_000];
        synth.render(&mut first);
        synth.trigger(Sound::Click, Strength::Strong);
        let mut second = vec![0.0; 1_000];
        synth.render(&mut second);
        assert_eq!(first, second);
    }

    #[test]
    fn a_voice_that_was_never_triggered_is_silent() {
        let mut synth = Synth::new(SR);
        let mut out = vec![0.0; 100];
        synth.render(&mut out);
        assert_eq!(peak(&out), 0.0);
    }
}
