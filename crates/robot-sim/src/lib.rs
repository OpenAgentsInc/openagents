//! Fixed-step Robot Lab execution. One backend owns every task-relevant body.

#[cfg(feature = "mujoco")]
pub mod import;
pub mod jobs;
#[cfg(feature = "mujoco")]
pub mod mujoco;
pub mod runner;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("The simulation couldn't read or save this file: {0}")]
    Io(#[from] std::io::Error),
    #[error("The simulation couldn't read this file: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Lab(#[from] robot_lab::Error),
    #[error("MuJoCo couldn't start: {0}")]
    Library(String),
}

pub(crate) fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::Invalid(message.into()))
    }
}
