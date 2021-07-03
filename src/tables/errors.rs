use crate::tables::emoji_tables::{EmojiTableKey, EmojiTable};

/// A representation of errors encountered while parsing or using emoji tables.
#[derive(Debug)]
pub enum EmojiTableError {
    /// Indicates that an emoji with the given sequence is not in the table
    KeyNotFound(EmojiTableKey),
}

#[derive(Debug)]
/// An error that occurs while expanding an [EmojiTable]
pub enum ExpansionError {
    /// Wrapper for [std::io::Error]
    Io(std::io::Error),
    /// Wrapper for multiple errors
    Multiple(Vec<ExpansionError>),
    #[cfg(feature = "online")]
    /// Wrappter for [reqwest::Error]
    Reqwest(reqwest::Error),
}

impl From<std::io::Error> for ExpansionError {
    fn from(err: std::io::Error) -> Self {
        ExpansionError::Io(err)
    }
}

impl From<Vec<ExpansionError>> for ExpansionError {
    fn from(errors: Vec<ExpansionError>) -> Self {
        Self::Multiple(errors)
    }
}

#[cfg(feature = "online")]
impl From<reqwest::Error> for ExpansionError {
    fn from(err: reqwest::Error) -> Self {
        ExpansionError::Reqwest(err)
    }
}
