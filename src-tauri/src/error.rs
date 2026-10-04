use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Validation(String),
    NotFound(&'static str),
    Io(std::io::Error),
    /// Screen capture, screenshot files or windows of the capture flow.
    Capture(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Validation(message) => f.write_str(message),
            AppError::NotFound(message) => f.write_str(message),
            AppError::Capture(message) => f.write_str(message),
            AppError::Io(error) => write!(f, "Не удалось сохранить конфигурацию: {error}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        AppError::Io(error)
    }
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

pub type AppResult<T> = Result<T, AppError>;
