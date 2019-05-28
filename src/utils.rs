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
