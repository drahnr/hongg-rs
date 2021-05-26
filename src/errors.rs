/// Wrapping error type.
#[derive(thiserror::Error, Debug)]
pub enum Error{
    #[error("Retrieving env error failed")]
    EnvVar(#[source] std::env::VarError),

    #[error(transparent)]
    Io(std::io::Error),

    #[error(transparent)]
    Which(which::Error),
}

/// Convenience result type.
pub type Result<T> = ::std::result::Result<T, Error>;
