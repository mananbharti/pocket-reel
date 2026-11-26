//! Pocket Reel conversion engine.
//!
//! * [`presets`]: the preset catalog (classic players, modern formats, audio).
//! * [`plan`]: settings + media -> concrete plan, FFmpeg arguments, estimates.
//! * [`queue`]: the batch queue with progress, ETA and cancellation.
//! * [`jpeg`], [`adpcm`], [`avi`], [`classic`]: the from-scratch encoder that
//!   reproduces the original converter's MJPEG/IMA-ADPCM AVI format.

pub mod adpcm;
pub mod avi;
pub mod classic;
pub mod error;
pub mod jpeg;
pub mod plan;
pub mod presets;
pub mod preview;
pub mod probe;
pub mod progress;
pub mod queue;
pub mod run;
pub mod tools;

pub use error::{Error, Result};
pub use plan::{Estimate, Plan, Settings};
pub use presets::{Fit, Preset};
pub use probe::MediaInfo;
pub use progress::{CancelToken, Progress};
pub use queue::{Engine, Event, JobId, JobRequest, JobSnapshot, JobState};
pub use tools::Tools;

/// Human-readable byte size (decimal units, like phones and file managers).
pub fn format_bytes(bytes: u64) -> String {
    let b = bytes as f64;
    if b < 1000.0 {
        format!("{bytes} B")
    } else if b < 1_000_000.0 {
        format!("{:.0} KB", b / 1000.0)
    } else if b < 1_000_000_000.0 {
        format!("{:.1} MB", b / 1_000_000.0)
    } else {
        format!("{:.2} GB", b / 1_000_000_000.0)
    }
}
