use failure::Fail;

pub type GenResult<T> = std::result::Result<T, failure::Error>;

#[derive(Fail, Debug)]
#[fail(display = "process exist code not zero")]
pub struct ExitCodeNotZero;
