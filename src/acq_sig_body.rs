// ALPHA-Ω synthetic acquisition and signal processing.
// Copyright (C) 2025–2026 ALPHA-Ω contributors
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// RECONSTRUCTED 2026-10-23: see the note in domain_body.rs. The
// driver contract is src/main.rs: a seeded sample stream of length
// `duration`, a sliding WINDOW-sample spectral estimate returned from
// every push once the window is full, and a dropped-sample counter.

use std::collections::VecDeque;

/// Deterministic 64-bit linear congruential generator (Knuth MMIX
/// constants). The sole source of randomness in the engine; a fixed
/// seed reproduces the full stream bit for bit.
pub struct Lcg(u64);

impl Lcg {
    pub fn new(seed: u64) -> Lcg {
        Lcg(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    /// Uniform draw in [0, 1) from the generator's top 53 bits.
    pub fn next_f64(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64) / ((1u64 << 53) as f64)
    }
}

/// Seeded synthetic alpha-band source. Emits one `Observation` per
/// sample; a dropped sample is emitted with `valid = false` and a zero
/// value (zero-fill dropout), never silently skipped, so downstream
/// audit sees exactly what the stream contained.
pub struct SyntheticAcquirer {
    cfg: AcqConfig,
    lcg: Lcg,
    index: u64,
    dropped: u64,
}

impl SyntheticAcquirer {
    pub fn new(cfg: AcqConfig) -> Result<SyntheticAcquirer, AlphaError> {
        if cfg.sample_rate == 0 {
            return Err(AlphaError("sample_rate must be positive".to_string()));
        }
        if cfg.duration == 0 {
            return Err(AlphaError("duration must be positive".to_string()));
        }
        if !cfg.alpha_freq.is_finite() || cfg.alpha_freq <= 0.0 {
            return Err(AlphaError("alpha_freq must be positive and finite".to_string()));
        }
        if !cfg.amplitude.is_finite() || !cfg.noise_std.is_finite() || cfg.noise_std < 0.0 {
            return Err(AlphaError(
                "amplitude / noise_std must be finite, noise_std >= 0".to_string(),
            ));
        }
        if cfg.drop_permille > 1000 {
            return Err(AlphaError("drop_permille must be <= 1000".to_string()));
        }
        Ok(SyntheticAcquirer {
            lcg: Lcg::new(cfg.seed),
            cfg,
            index: 0,
            dropped: 0,
        })
    }

    /// Next sample in the stream; `Err` once `duration` samples have
    /// been emitted (end of stream).
    pub fn next(&mut self) -> Result<Observation, AlphaError> {
        if self.index >= self.cfg.duration {
            return Err(AlphaError("end of stream".to_string()));
        }
        let idx = self.index;
        self.index += 1;

        let rate = self.cfg.sample_rate as f64;
        let timestamp_ns = idx.wrapping_mul(1_000_000_000u64 / self.cfg.sample_rate as u64);

        // Drop draw first: a dropped sample consumes no noise draws,
        // keeping the surviving stream a pure function of the seed.
        let dropped_now = self.lcg.next_f64() < (self.cfg.drop_permille as f64) / 1000.0;

        // Gaussian noise by Box-Muller from two uniform draws.
        let u1 = self.lcg.next_f64().max(1e-12);
        let u2 = self.lcg.next_f64();
        let noise =
            self.cfg.noise_std * (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();

        let t = idx as f64 / rate;
        let clean = self.cfg.amplitude
            * (2.0 * std::f64::consts::PI * self.cfg.alpha_freq * t).sin();

        let (sample, valid) = if dropped_now {
            self.dropped += 1;
            (0.0, false)
        } else {
            (clean + noise, true)
        };

        let mut obs = Observation {
            timestamp_ns,
            sample_index: idx,
            sample,
            alpha_power: Fixed::ZERO,
            quality: self.cfg.quality_base,
            valid,
            band_detected: (ALPHA_BAND_MIN_HZ..=ALPHA_BAND_MAX_HZ).contains(&self.cfg.alpha_freq),
            checksum: 0,
        };
        obs.checksum = Observation::compute_checksum(
            obs.timestamp_ns,
            obs.sample_index,
            obs.alpha_power,
            obs.quality,
            obs.valid,
        );
        Ok(obs)
    }

    /// Number of dropped samples emitted so far.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

/// Windowed spectral processor. Keeps the last `WINDOW` samples and,
/// once the window is full, returns on every push a Goertzel estimate
/// at `ALPHA_TARGET_HZ` as (power, quality, band_detected):
/// power is the squared amplitude at the target bin, quality is the
/// spectral concentration scaled to 0..=100, and band_detected reports
/// whether that concentration clears the detection floor.
pub struct SignalProcessor {
    rate: u32,
    window: VecDeque<f64>,
    last_index: Option<u64>,
}

impl SignalProcessor {
    pub fn new(rate: u32) -> SignalProcessor {
        SignalProcessor {
            rate,
            window: VecDeque::with_capacity(WINDOW),
            last_index: None,
        }
    }

    pub fn push(
        &mut self,
        obs: &Observation,
    ) -> Result<Option<(Fixed, u8, bool)>, AlphaError> {
        if !obs.sample.is_finite() {
            return Err(AlphaError(format!(
                "non-finite sample at index {}",
                obs.sample_index
            )));
        }
        if let Some(prev) = self.last_index {
            if obs.sample_index != prev + 1 {
                return Err(AlphaError(format!(
                    "sample index gap: expected {}, got {}",
                    prev + 1,
                    obs.sample_index
                )));
            }
        }
        self.last_index = Some(obs.sample_index);

        self.window.push_back(obs.sample);
        if self.window.len() > WINDOW {
            self.window.pop_front();
        }
        if self.window.len() < WINDOW {
            return Ok(None);
        }

        // Goertzel single-bin DFT at the alpha target frequency.
        let n = WINDOW as f64;
        let k = (ALPHA_TARGET_HZ * n / self.rate as f64).round();
        let w = 2.0 * std::f64::consts::PI * k / n;
        let coeff = 2.0 * w.cos();
        let (mut s1, mut s2) = (0.0f64, 0.0f64);
        let mut mean_sq = 0.0f64;
        for &x in self.window.iter() {
            let s = x + coeff * s1 - s2;
            s2 = s1;
            s1 = s;
            mean_sq += x * x;
        }
        mean_sq /= n;
        let power_raw = s1 * s1 + s2 * s2 - coeff * s1 * s2;
        // Squared amplitude at the bin; a pure in-band sine of
        // amplitude A yields A^2 (plus a small noise projection).
        let amp2 = (2.0 / n) * (2.0 / n) * power_raw.max(0.0);
        // Concentration: bin power against total signal power
        // (a pure sine splits evenly, hence the factor of two).
        let concentration = if mean_sq > 0.0 {
            (amp2 / (2.0 * mean_sq)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let quality = (100.0 * concentration).round().clamp(0.0, 100.0) as u8;
        let band_detected = concentration >= 0.25;
        Ok(Some((Fixed::from_f64(amp2), quality, band_detected)))
    }
}

#[cfg(test)]
mod acq_tests {
    use super::*;

    /// Cross-language reference for the Modula-2 alternate numerics
    /// kernel (modula2/AlphaNumerics.mod): a pure 10 Hz sine of
    /// amplitude 1 at 256 Hz, no noise, must measure exactly the
    /// full-scale fixed-point power 100_000 (= 1.0).
    #[test]
    fn processor_pure_sine_power() {
        let mut proc = SignalProcessor::new(DEFAULT_RATE);
        let mut result = None;
        for i in 0..WINDOW {
            let sample = (2.0 * std::f64::consts::PI * ALPHA_TARGET_HZ * i as f64
                / DEFAULT_RATE as f64)
                .sin();
            let obs = Observation {
                timestamp_ns: i as u64 * 3_906_250,
                sample_index: i as u64,
                sample,
                alpha_power: Fixed::ZERO,
                quality: 0,
                valid: true,
                band_detected: true,
                checksum: 0,
            };
            result = proc.push(&obs).expect("push");
        }
        let (power, quality, band) = result.expect("window must be full");
        assert_eq!(power, Fixed(100_000));
        assert_eq!(quality, 100);
        assert!(band);
    }
}
