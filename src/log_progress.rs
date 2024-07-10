use bdk::{blockchain::Progress, Error};

/// Type that implements [`Progress`] and logs at level `INFO` every update received
#[derive(Clone, Copy, Default, Debug)]
pub struct LogProgress;

/// Create a new instance of [`LogProgress`]
pub fn log_progress() -> LogProgress {
    LogProgress
}

impl Progress for LogProgress {
    fn update(&self, progress: f32, message: Option<String>) -> Result<(), Error> {
        eprintln!(
            "Sync {:.3}%: `{}`",
            progress,
            message.unwrap_or_else(|| "".into())
        );

        Ok(())
    }
}
