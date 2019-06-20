use crate::errors::{ExitCodeNotZero, GenResult};

pub trait CommandExt {
    fn checked_call(&mut self) -> GenResult<()>;
}

impl CommandExt for std::process::Command {
    fn checked_call(&mut self) -> GenResult<()> {
        if !self.status()?.success() {
            return Err(ExitCodeNotZero.into());
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct BytesSize(pub usize);

impl std::fmt::Display for BytesSize {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.0 < 2usize.pow(10) {
            write!(f, "{} B", self.0)
        } else if self.0 < 2usize.pow(20) {
            write!(f, "{:.3} KB", self.0 as f64 / 2f64.powf(10.0))
        } else if self.0 < 2usize.pow(30) {
            write!(f, "{:.3} MB", self.0 as f64 / 2f64.powf(20.0))
        } else {
            write!(f, "{:.3} GB", self.0 as f64 / 2f64.powf(30.0))
        }
    }
}
