use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0} was not found. Pocket Reel needs FFmpeg to read and convert media.")]
    ToolMissing(&'static str),
    #[error("{message}")]
    Probe { path: PathBuf, message: String },
    #[error("This file has no {0} stream.")]
    MissingStream(&'static str),
    #[error("Unknown preset \"{0}\".")]
    UnknownPreset(String),
    #[error("{0}")]
    Invalid(String),
    #[error("This preset needs the {0} encoder, which this FFmpeg build does not include.")]
    EncoderMissing(String),
    #[error("Conversion failed: {0}")]
    Ffmpeg(String),
    #[error("Canceled")]
    Canceled,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
