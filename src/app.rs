use crate::decoder::decode_audio_file;
use crate::error::{AppError, AppResult};
use crate::generator::{GeneratorConfig, WaveGenerator};
use crate::gui::run_gui;
use crate::render::{print_analysis, write_spectrum_csv};
use crate::spectrum::{SpectrumConfig, SpectrumStrategy, WindowKind};
use crate::wav::write_wav_file;
use std::path::PathBuf;

const DEFAULT_SPECTRUM_BINS: usize = 48;
const DEFAULT_WINDOW_SIZE: usize = 2048;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Analyze(AnalyzeOptions),
    Generate(GenerateOptions),
    Gui(GuiOptions),
    Help,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyzeOptions {
    pub input: PathBuf,
    pub bins: usize,
    pub window_size: usize,
    pub window_kind: WindowKind,
    pub csv_output: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenerateOptions {
    pub output: PathBuf,
    pub seconds: f32,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiOptions {
    pub input: Option<PathBuf>,
}

pub fn run<I, S>(args: I) -> AppResult<()>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    match parse_args(args)? {
        Command::Analyze(options) => analyze(options),
        Command::Generate(options) => generate(options),
        Command::Gui(options) => run_gui(options.input),
        Command::Help => {
            println!("{}", help_text());
            Ok(())
        }
    }
}

pub fn parse_args<I, S>(args: I) -> AppResult<Command>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut iter = args.into_iter().map(Into::into);
    let _program = iter.next();
    let Some(command) = iter.next() else {
        return Ok(Command::Help);
    };

    match command.as_str() {
        "analyze" => parse_analyze(iter.collect()),
        "generate" => parse_generate(iter.collect()),
        "gui" => parse_gui(iter.collect()),
        "help" | "--help" | "-h" => Ok(Command::Help),
        other => Err(AppError::invalid_argument(format!(
            "unknown command '{other}', try 'help'"
        ))),
    }
}

fn parse_gui(args: Vec<String>) -> AppResult<Command> {
    match args.as_slice() {
        [] => Ok(Command::Gui(GuiOptions { input: None })),
        [path] => Ok(Command::Gui(GuiOptions {
            input: Some(PathBuf::from(path)),
        })),
        _ => Err(AppError::invalid_argument(
            "gui accepts zero or one input path",
        )),
    }
}

fn parse_analyze(args: Vec<String>) -> AppResult<Command> {
    if args.is_empty() {
        return Err(AppError::invalid_argument(
            "analyze requires an input WAV path",
        ));
    }

    let mut input = None;
    let mut bins = DEFAULT_SPECTRUM_BINS;
    let mut window_size = DEFAULT_WINDOW_SIZE;
    let mut window_kind = WindowKind::Hann;
    let mut csv_output = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--bins" => {
                index += 1;
                bins = parse_usize_arg(args.get(index), "--bins")?;
            }
            "--window" => {
                index += 1;
                window_size = parse_usize_arg(args.get(index), "--window")?;
            }
            "--window-kind" => {
                index += 1;
                window_kind = parse_window_kind(args.get(index))?;
            }
            "--csv" => {
                index += 1;
                let path = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_argument("--csv requires a path"))?;
                csv_output = Some(PathBuf::from(path));
            }
            value if value.starts_with('-') => {
                return Err(AppError::invalid_argument(format!(
                    "unknown analyze option '{value}'"
                )));
            }
            value => {
                if input.replace(PathBuf::from(value)).is_some() {
                    return Err(AppError::invalid_argument(
                        "analyze accepts only one input path",
                    ));
                }
            }
        }
        index += 1;
    }

    let input = input.ok_or_else(|| AppError::invalid_argument("missing input WAV path"))?;
    validate_bins(bins)?;
    validate_window(window_size)?;

    Ok(Command::Analyze(AnalyzeOptions {
        input,
        bins,
        window_size,
        window_kind,
        csv_output,
    }))
}

fn parse_generate(args: Vec<String>) -> AppResult<Command> {
    if args.is_empty() {
        return Err(AppError::invalid_argument(
            "generate requires an output WAV path",
        ));
    }

    let mut output = None;
    let mut seconds = 3.0_f32;
    let mut sample_rate = 44_100_u32;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--seconds" => {
                index += 1;
                seconds = parse_f32_arg(args.get(index), "--seconds")?;
            }
            "--rate" => {
                index += 1;
                sample_rate = parse_u32_arg(args.get(index), "--rate")?;
            }
            value if value.starts_with('-') => {
                return Err(AppError::invalid_argument(format!(
                    "unknown generate option '{value}'"
                )));
            }
            value => {
                if output.replace(PathBuf::from(value)).is_some() {
                    return Err(AppError::invalid_argument(
                        "generate accepts only one output path",
                    ));
                }
            }
        }
        index += 1;
    }

    if !(0.1..=30.0).contains(&seconds) {
        return Err(AppError::invalid_argument(
            "--seconds must be between 0.1 and 30.0",
        ));
    }
    if !(8_000..=192_000).contains(&sample_rate) {
        return Err(AppError::invalid_argument(
            "--rate must be between 8000 and 192000",
        ));
    }

    Ok(Command::Generate(GenerateOptions {
        output: output.ok_or_else(|| AppError::invalid_argument("missing output WAV path"))?,
        seconds,
        sample_rate,
    }))
}

fn parse_usize_arg(value: Option<&String>, name: &str) -> AppResult<usize> {
    value
        .ok_or_else(|| AppError::invalid_argument(format!("{name} requires a value")))?
        .parse::<usize>()
        .map_err(|_| AppError::invalid_argument(format!("{name} must be a positive integer")))
}

fn parse_u32_arg(value: Option<&String>, name: &str) -> AppResult<u32> {
    value
        .ok_or_else(|| AppError::invalid_argument(format!("{name} requires a value")))?
        .parse::<u32>()
        .map_err(|_| AppError::invalid_argument(format!("{name} must be a positive integer")))
}

fn parse_f32_arg(value: Option<&String>, name: &str) -> AppResult<f32> {
    value
        .ok_or_else(|| AppError::invalid_argument(format!("{name} requires a value")))?
        .parse::<f32>()
        .map_err(|_| AppError::invalid_argument(format!("{name} must be a number")))
}

fn parse_window_kind(value: Option<&String>) -> AppResult<WindowKind> {
    match value.map(String::as_str) {
        Some("hann") => Ok(WindowKind::Hann),
        Some("rect") | Some("rectangular") => Ok(WindowKind::Rectangular),
        Some(other) => Err(AppError::invalid_argument(format!(
            "--window-kind must be 'hann' or 'rectangular', got '{other}'"
        ))),
        None => Err(AppError::invalid_argument("--window-kind requires a value")),
    }
}

fn validate_bins(bins: usize) -> AppResult<()> {
    if (8..=120).contains(&bins) {
        Ok(())
    } else {
        Err(AppError::invalid_argument(
            "--bins must be between 8 and 120",
        ))
    }
}

fn validate_window(window_size: usize) -> AppResult<()> {
    if window_size.is_power_of_two() && (128..=16_384).contains(&window_size) {
        Ok(())
    } else {
        Err(AppError::invalid_argument(
            "--window must be a power of two from 128 to 16384",
        ))
    }
}

fn analyze(options: AnalyzeOptions) -> AppResult<()> {
    let audio = decode_audio_file(&options.input)?;
    let config = SpectrumConfig::new(options.window_size, options.bins)?;
    let spectrum = SpectrumStrategy::with_window(options.window_kind).analyze(&audio, &config);
    let analysis = audio.analyze(&spectrum);
    let waveform = audio.waveform_preview(48);

    print_analysis(&analysis, &spectrum, &waveform);

    if let Some(path) = options.csv_output {
        write_spectrum_csv(&path, &spectrum)?;
        println!("\nCSV spectrum written to {}", path.display());
    }

    Ok(())
}

fn generate(options: GenerateOptions) -> AppResult<()> {
    let generator = WaveGenerator::default();
    let config = GeneratorConfig {
        sample_rate: options.sample_rate,
        seconds: options.seconds,
    };
    let wav = generator.demo_wave(config);
    write_wav_file(&options.output, &wav)?;
    println!(
        "Generated demo WAV: {} ({:.1}s, {} Hz)",
        options.output.display(),
        wav.duration_seconds(),
        options.sample_rate
    );
    Ok(())
}

pub fn help_text() -> &'static str {
    "WaveScope - a small Rust WAV spectrum analyzer\n\
\n\
USAGE:\n\
  cargo run -- analyze <input-audio> [--bins 48] [--window 2048] [--window-kind hann] [--csv spectrum.csv]\n\
  cargo run -- generate <output.wav> [--seconds 3.0] [--rate 44100]\n\
  cargo run -- gui [input-audio]\n\
\n\
COMMANDS:\n\
  analyze    Read a common audio file and print amplitude/frequency statistics\n\
  generate   Create a small demo WAV with bass, melody and sweep tones\n\
  gui        Open the desktop spectrum viewer with optional synchronized playback\n\
  help       Show this help text\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_analyze_defaults() {
        let command = parse_args(["wavescope", "analyze", "voice.wav"]).unwrap();
        assert_eq!(
            command,
            Command::Analyze(AnalyzeOptions {
                input: PathBuf::from("voice.wav"),
                bins: DEFAULT_SPECTRUM_BINS,
                window_size: DEFAULT_WINDOW_SIZE,
                window_kind: WindowKind::Hann,
                csv_output: None,
            })
        );
    }

    #[test]
    fn parse_generate_options() {
        let command = parse_args([
            "wavescope",
            "generate",
            "demo.wav",
            "--seconds",
            "2.5",
            "--rate",
            "48000",
        ])
        .unwrap();
        assert_eq!(
            command,
            Command::Generate(GenerateOptions {
                output: PathBuf::from("demo.wav"),
                seconds: 2.5,
                sample_rate: 48_000,
            })
        );
    }

    #[test]
    fn parse_gui_with_path() {
        let command = parse_args(["wavescope", "gui", "song.mp3"]).unwrap();
        assert_eq!(
            command,
            Command::Gui(GuiOptions {
                input: Some(PathBuf::from("song.mp3"))
            })
        );
    }

    #[test]
    fn reject_non_power_of_two_window() {
        let result = parse_args(["wavescope", "analyze", "a.wav", "--window", "1000"]);
        assert!(result.is_err());
    }
}
