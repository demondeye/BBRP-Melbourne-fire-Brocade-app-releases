//! Startup chime — an original chord synthesised in code (no audio file),
//! the same voicing and shape as the web version.

use std::f32::consts::TAU;

const RATE: u32 = 44_100;
const LENGTH: f32 = 4.5;

fn render() -> Vec<f32> {
    // Wide D major add9 voicing: D2 A2 D3 F#3 A3 D4 E4
    let notes = [73.42f32, 110.0, 146.83, 185.0, 220.0, 293.66, 329.63];
    let total = ((LENGTH + 0.15) * RATE as f32) as usize;
    let mut out = vec![0.0f32; total];
    // Each note: a triangle plus a quiet, slightly detuned sawtooth.
    for (i, &freq) in notes.iter().enumerate() {
        let scale = 1.0 / (1.0 + i as f32 * 0.25);
        let detune = if i % 2 == 1 { 6.0 } else { -6.0 };
        let saw_freq = freq * 2f32.powf(detune / 1200.0);
        for (n, sample) in out.iter_mut().enumerate() {
            let t = n as f32 / RATE as f32;
            let p = (t * freq).fract();
            let tri = 1.0 - 4.0 * (p - 0.5).abs();
            let saw = 2.0 * (t * saw_freq).fract() - 1.0;
            *sample += (tri * 0.5 + saw * 0.12) * scale;
        }
    }
    // Low-pass filter sweeping 2400 Hz → 500 Hz, and the volume envelope.
    let mut y = 0.0f32;
    for (n, sample) in out.iter_mut().enumerate() {
        let t = n as f32 / RATE as f32;
        let cutoff = 2400.0 * (500.0f32 / 2400.0).powf((t / LENGTH).min(1.0));
        let a = 1.0 - (-TAU * cutoff / RATE as f32).exp();
        y += a * (*sample - y);
        let gain = if t < 0.04 {
            0.0001 * (0.32f32 / 0.0001).powf(t / 0.04)
        } else if t < LENGTH {
            0.32 * (0.0001f32 / 0.32).powf((t - 0.04) / (LENGTH - 0.04))
        } else {
            0.0
        };
        *sample = (y * gain * 0.6).clamp(-1.0, 1.0);
    }
    out
}

/// Plays in the background; if there's no sound device it boots silently.
pub fn play() {
    std::thread::spawn(|| {
        let Ok((_stream, handle)) = rodio::OutputStream::try_default() else { return };
        let Ok(sink) = rodio::Sink::try_new(&handle) else { return };
        sink.append(rodio::buffer::SamplesBuffer::new(1, RATE, render()));
        sink.sleep_until_end();
    });
}
