use crate::error::AppResult;
use crate::signal::AudioAnalysis;
use crate::spectrum::{Spectrum, SpectrumBin};
use std::fs::File;
use std::io::Write;
use std::path::Path;

const BAR_WIDTH: usize = 42;

pub fn print_analysis(analysis: &AudioAnalysis, spectrum: &Spectrum, waveform: &[f32]) {
    println!("WaveScope Analysis");
    println!("==================");
    println!("Sample rate : {} Hz", analysis.sample_rate);
    println!("Channels    : {}", analysis.channels);
    println!("Frames      : {}", analysis.frames);
    println!("Duration    : {:.3} s", analysis.duration_seconds);
    println!(
        "Dominant    : {:.1} Hz (magnitude {:.5})",
        analysis.dominant_frequency_hz, analysis.dominant_magnitude
    );

    println!("\nChannel statistics");
    for stats in &analysis.channel_stats {
        println!(
            "  ch {:>2}: RMS {:>7.4}, peak {:>7.4}, mean {:>8.5}, crest {:>5.2}, zero crossings {}",
            stats.channel_index + 1,
            stats.rms,
            stats.peak,
            stats.mean,
            stats.crest_factor,
            stats.zero_crossings
        );
    }

    if !waveform.is_empty() {
        println!("\nWaveform preview");
        println!("{}", render_waveform(waveform));
    }

    println!("\nSpectrum");
    for line in render_spectrum_lines(spectrum) {
        println!("{line}");
    }
}

pub fn render_waveform(samples: &[f32]) -> String {
    samples
        .iter()
        .map(|sample| match *sample {
            value if value > 0.66 => '^',
            value if value > 0.2 => '+',
            value if value < -0.66 => 'v',
            value if value < -0.2 => '-',
            _ => '.',
        })
        .collect()
}

pub fn render_spectrum_lines(spectrum: &Spectrum) -> Vec<String> {
    let max_magnitude = spectrum
        .bins
        .iter()
        .map(|bin| bin.magnitude)
        .fold(0.0_f32, f32::max);
    spectrum
        .bins
        .iter()
        .map(|bin| render_bin(bin, max_magnitude))
        .collect()
}

pub fn write_spectrum_csv(path: &Path, spectrum: &Spectrum) -> AppResult<()> {
    let mut file = File::create(path)?;
    writeln!(file, "index,start_hz,end_hz,center_hz,magnitude")?;
    for bin in &spectrum.bins {
        writeln!(
            file,
            "{},{:.3},{:.3},{:.3},{:.8}",
            bin.index, bin.start_hz, bin.end_hz, bin.center_hz, bin.magnitude
        )?;
    }
    Ok(())
}

fn render_bin(bin: &SpectrumBin, max_magnitude: f32) -> String {
    let ratio = if max_magnitude <= f32::EPSILON {
        0.0
    } else {
        bin.magnitude / max_magnitude
    };
    let filled = (ratio * BAR_WIDTH as f32).round() as usize;
    let bar = "#".repeat(filled);
    format!(
        "{:>7.0}-{:>7.0} Hz | {:<width$} {:.5}",
        bin.start_hz,
        bin.end_hz,
        bar,
        bin.magnitude,
        width = BAR_WIDTH
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_keeps_one_line_per_bin() {
        let spectrum = Spectrum {
            max_frequency_hz: 1000.0,
            bins: vec![
                SpectrumBin {
                    index: 0,
                    start_hz: 0.0,
                    end_hz: 500.0,
                    center_hz: 250.0,
                    magnitude: 0.5,
                },
                SpectrumBin {
                    index: 1,
                    start_hz: 500.0,
                    end_hz: 1000.0,
                    center_hz: 750.0,
                    magnitude: 1.0,
                },
            ],
        };
        let lines = render_spectrum_lines(&spectrum);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains('#'));
    }

    #[test]
    fn waveform_preview_uses_directional_symbols() {
        assert_eq!(render_waveform(&[-1.0, -0.4, 0.0, 0.4, 1.0]), "v-.+^");
    }
}
