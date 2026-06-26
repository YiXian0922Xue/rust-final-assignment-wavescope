use std::fmt::{Display, Formatter};
use std::io;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug)]
pub enum AppError {
    Io(io::Error),
    Decode(String),
    Playback(String),
    Gui(String),
    InvalidArgument(String),
    InvalidWav(String),
    UnsupportedWav(String),
}

impl AppError {
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::InvalidArgument(message.into())
    }

    pub fn decode(message: impl Into<String>) -> Self {
        Self::Decode(message.into())
    }

    pub fn playback(message: impl Into<String>) -> Self {
        Self::Playback(message.into())
    }

    pub fn gui(message: impl Into<String>) -> Self {
        Self::Gui(message.into())
    }

    pub fn invalid_wav(message: impl Into<String>) -> Self {
        Self::InvalidWav(message.into())
    }

    pub fn unsupported_wav(message: impl Into<String>) -> Self {
        Self::UnsupportedWav(message.into())
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Io(err) => write!(f, "I/O failed: {err}"),
            AppError::Decode(message) => write!(f, "audio decode failed: {message}"),
            AppError::Playback(message) => write!(f, "audio playback failed: {message}"),
            AppError::Gui(message) => write!(f, "GUI failed: {message}"),
            AppError::InvalidArgument(message) => write!(f, "invalid argument: {message}"),
            AppError::InvalidWav(message) => write!(f, "invalid WAV file: {message}"),
            AppError::UnsupportedWav(message) => write!(f, "unsupported WAV format: {message}"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for AppError {
    fn from(value: io::Error) -> Self {
        AppError::Io(value)
    }
}

impl From<symphonia::core::errors::Error> for AppError {
    fn from(value: symphonia::core::errors::Error) -> Self {
        AppError::Decode(value.to_string())
    }
}
