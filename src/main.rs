mod app;
mod decoder;
mod eq;
mod error;
mod fft_spectrum;
mod generator;
mod gui;
mod player;
mod render;
mod signal;
mod spectrum;
mod wav;

fn main() {
    if let Err(err) = app::run(std::env::args()) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
