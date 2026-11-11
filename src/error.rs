use std::fmt;

/// Application errors. Phase 0 keeps this small: terminal, renderer, IO.
#[derive(Debug)]
pub enum AppError {
    Terminal(String),
    Renderer(String),
    Io(std::io::Error),
    UnsupportedTerminal,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Terminal(msg) => write!(f, "terminal: {msg}"),
            AppError::Renderer(msg) => write!(f, "renderer: {msg}"),
            AppError::Io(err) => write!(f, "io: {err}"),
            AppError::UnsupportedTerminal => write!(f, "terminal not supported"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err)
    }
}
