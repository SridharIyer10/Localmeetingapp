//! Echo suppression (Chunk 2, pure part).
//!
//! Without headphones the user's own voice returns on the loopback channel,
//! which corrupts speaker attribution. We estimate the delay between mic and
//! loopback by normalized cross-correlation and, when the correlation is strong
//! enough, subtract a single-tap estimate of the echo. The full app also has the
//! headphone fallback; this is the algorithmic first cut.

/// Deterministic pseudo-random signal for tests and calibration (splitmix64).
#[cfg(test)]
fn lcg_signal(len: usize, seed: u64) -> Vec<f32> {
    let mut z = seed;
    (0..len)
        .map(|_| {
            z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            x ^= x >> 31;
            (x as f64 / u64::MAX as f64) as f32 - 0.5
        })
        .collect()
}

/// Normalized cross-correlation of `mic` against `loopback` at a given lag,
/// where `loopback` index is `i + lag`.
pub fn normalized_correlation(mic: &[f32], loopback: &[f32], lag: isize) -> f32 {
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    let n = mic.len() as isize;
    for i in 0..n {
        let j = i + lag;
        if j < 0 || j >= loopback.len() as isize {
            continue;
        }
        let a = mic[i as usize];
        let b = loopback[j as usize];
        dot += a * b;
        na += a * a;
        nb += b * b;
    }
    if na <= 0.0 || nb <= 0.0 {
        0.0
    } else {
        (dot / (na.sqrt() * nb.sqrt())).clamp(-1.0, 1.0)
    }
}

/// Find the lag with the strongest (absolute) correlation in `[-max_lag, max_lag]`.
pub fn estimate_delay(mic: &[f32], loopback: &[f32], max_lag: usize) -> (isize, f32) {
    let mut best_lag = 0isize;
    let mut best = 0.0f32;
    for lag in -(max_lag as isize)..=(max_lag as isize) {
        let c = normalized_correlation(mic, loopback, lag).abs();
        if c > best {
            best = c;
            best_lag = lag;
        }
    }
    (best_lag, best)
}

/// True when the loopback is plausibly an echo of the mic.
pub fn echo_present(mic: &[f32], loopback: &[f32], max_lag: usize, threshold: f32) -> bool {
    let (_, peak) = estimate_delay(mic, loopback, max_lag);
    peak >= threshold
}

/// Subtract a single-tap echo estimate from the mic signal. Returns the mic
/// unchanged when no echo is detected.
pub fn suppress(mic: &[f32], loopback: &[f32], max_lag: usize, threshold: f32) -> Vec<f32> {
    let (lag, peak) = estimate_delay(mic, loopback, max_lag);
    if peak < threshold {
        return mic.to_vec();
    }
    // Least-squares gain for the aligned loopback signal.
    let mut num = 0.0f32;
    let mut den = 0.0f32;
    for i in 0..mic.len() as isize {
        let j = i + lag;
        if j < 0 || j >= loopback.len() as isize {
            continue;
        }
        let b = loopback[j as usize];
        num += mic[i as usize] * b;
        den += b * b;
    }
    let gain = if den > 0.0 { num / den } else { 0.0 };

    let mut out = mic.to_vec();
    for i in 0..out.len() as isize {
        let j = i + lag;
        if j < 0 || j >= loopback.len() as isize {
            continue;
        }
        out[i as usize] = mic[i as usize] - gain * loopback[j as usize];
    }
    out
}

#[cfg(test)]
fn energy(s: &[f32]) -> f32 {
    s.iter().map(|x| x * x).sum()
}

#[cfg(test)]
fn delayed_echo(mic: &[f32], delay: usize, gain: f32) -> Vec<f32> {
    let mut loopback = vec![0.0f32; mic.len()];
    for i in delay..mic.len() {
        loopback[i] = gain * mic[i - delay];
    }
    loopback
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_known_delay() {
        let mic = lcg_signal(4000, 7);
        let loopback = delayed_echo(&mic, 20, 0.6);
        let (lag, peak) = estimate_delay(&mic, &loopback, 80);
        assert_eq!(lag, 20);
        assert!(peak > 0.9, "peak was {peak}");
    }

    #[test]
    fn no_echo_when_signals_unrelated() {
        let mic = lcg_signal(4000, 1);
        let other = lcg_signal(4000, 999);
        let (_, peak) = estimate_delay(&mic, &other, 80);
        assert!(peak < 0.5, "unrelated signals correlated at {peak}");
        assert!(!echo_present(&mic, &other, 80, 0.5));
    }

    #[test]
    fn suppression_reduces_echo_energy() {
        let mic = lcg_signal(4000, 42);
        let loopback = delayed_echo(&mic, 15, 0.5);
        assert!(echo_present(&mic, &loopback, 80, 0.7));
        let cleaned = suppress(&mic, &loopback, 80, 0.7);
        assert!(
            energy(&cleaned) <= energy(&mic),
            "suppression increased energy"
        );
        let residual = normalized_correlation(&cleaned, &loopback, 15).abs();
        assert!(residual < 0.6, "residual correlation {residual} too high");
    }

    #[test]
    fn suppression_is_a_noop_without_echo() {
        let mic = lcg_signal(1000, 5);
        let other = lcg_signal(1000, 6);
        assert_eq!(suppress(&mic, &other, 40, 0.5), mic);
    }
}
