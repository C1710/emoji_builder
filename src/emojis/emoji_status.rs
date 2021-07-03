use std::str::FromStr;

/// The status of an emoji according to `emoji-test.txt`
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum EmojiStatus {
    /// ? TODO: Find out, what this is
    Component,
    /// It is a regular, RGI emoji
    FullyQualified,
    /// ? TODO: Find out, what this is
    MinimallyQualified,
    /// Not actually displayed as an emoji/not RGI
    Unqualified
}

impl EmojiStatus {
    pub fn is_emoji(&self) -> bool {
        matches!(self, Self::Component | Self::FullyQualified | Self::MinimallyQualified)
    }
}

impl Default for EmojiStatus {
    fn default() -> Self {
        Self::Unqualified
    }
}

impl ToString for EmojiStatus {
    fn to_string(&self) -> String {
        match self {
            Self::Component => "component".to_string(),
            Self::Unqualified => "unqualified".to_string(),
            Self::FullyQualified => "fully-qualified".to_string(),
            Self::MinimallyQualified => "minimally-qualified".to_string()
        }
    }
}

impl FromStr for EmojiStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "component" => Ok(Self::Component),
            "unqualified" => Ok(Self::Unqualified),
            "fully-qualified" => Ok(Self::FullyQualified),
            "minimally-qualified" => Ok(Self::MinimallyQualified),
            other => Err(other.to_string())
        }
    }
}
