use core::cmp::{Ord, Ordering, PartialOrd};
use core::convert::From;
use core::option::Option;
use core::result::Result;
use core::result::Result::{Err, Ok};
use std::str::FromStr;
use itertools::Itertools;

/// An internal representation for the different emoji types represented in the Unicode® Tables
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum EmojiKind {
    /// A regular emoji
    Emoji,
    /// An ZWJ-sequence (a sequence containing `U+200D`)
    EmojiZwjSequence,
    /// A sequence of multiple characters
    EmojiSequence,
    /// Something that can be displayed as an emoji
    EmojiPresentation,
    /// Something that can be combined with a modifier
    ModifierBase,
    /// ???
    EmojiComponent,
    /// A sequence including the keycap character (no idea, why it exists)
    EmojiKeycapSequence,
    /// A flag
    EmojiFlagSequence,
    /// An emoji with a modifier (e.g. skin tone)
    EmojiModifierSequence,
    ExtendedPictographic,
    // TODO: delete.
    /// Something else, that is not mapped here
    Other(String),
}

impl FromStr for EmojiKind {
    type Err = UnknownEmojiKind;

    fn from_str(kind: &str) -> Result<Self, Self::Err> {
        let kind = kind.to_lowercase().replace("rgi", "").replace('_', " ");
        let kind = kind.trim();
        match kind {
            "emoji" => Ok(EmojiKind::Emoji),
            "basic emoji" => Ok(EmojiKind::Emoji),
            "emoji zwj sequence" => Ok(EmojiKind::EmojiZwjSequence),
            "emoji sequence" => Ok(EmojiKind::EmojiSequence),
            "emoji presentation" => Ok(EmojiKind::EmojiPresentation),
            "modifier base" => Ok(EmojiKind::ModifierBase),
            "emoji modifier base" => Ok(EmojiKind::ModifierBase),
            "emoji component" => Ok(EmojiKind::EmojiComponent),
            "emoji keycap sequence" => Ok(EmojiKind::EmojiKeycapSequence),
            "emoji flag sequence" => Ok(EmojiKind::EmojiFlagSequence),
            "emoji modifier sequence" => Ok(EmojiKind::EmojiModifierSequence),
            "extended pictographic" => Ok(EmojiKind::ExtendedPictographic),
            _ => Err(UnknownEmojiKind(EmojiKind::Other(kind.to_owned()))),
        }
    }
}

/// A very simple wrapper that indicates, that a given string representation of an Emoji kind did
/// not match any of the default cases.
/// If you don't care about that, you can simply ignore it.
/// # Examples
/// ```
/// use std::str::FromStr;
/// use emoji_builder::emojis::emoji_kind::EmojiKind;
///
/// let kind = EmojiKind::from_str(":P");
/// assert!(kind.is_err());
/// assert_eq!(EmojiKind::Other(String::from(":p")), kind.err().unwrap().get());
/// ```
pub struct UnknownEmojiKind(EmojiKind);

impl UnknownEmojiKind {
    /// Returns the unknown emoji kind (which will be of the type [EmojiKind::Other])
    pub fn get(self) -> EmojiKind {
        self.0
    }
}

impl From<UnknownEmojiKind> for EmojiKind {
    fn from(kind: UnknownEmojiKind) -> Self {
        kind.0
    }
}

impl ToString for EmojiKind {
    fn to_string(&self) -> String {
        match self {
            EmojiKind::Emoji => {"Emoji".to_string()}
            EmojiKind::EmojiZwjSequence => {"Emoji_ZWJ_Sequence".to_string()}
            EmojiKind::EmojiSequence => {"Emoji_Sequence".to_string()}
            EmojiKind::EmojiPresentation => {"Emoji_Presentation".to_string()}
            EmojiKind::ModifierBase => {"Emoji_Modifier_Base".to_string()}
            EmojiKind::EmojiComponent => {"Emoji_Component".to_string()}
            EmojiKind::EmojiKeycapSequence => {"Emoji_Keycap_Sequence".to_string()}
            EmojiKind::EmojiFlagSequence => {"Emoji_Flag_Sequence".to_string()}
            EmojiKind::EmojiModifierSequence => {"Emoji_Modifier_Sequence".to_string()},
            EmojiKind::ExtendedPictographic => {"Extended_Pictographic".to_string()},
            EmojiKind::Other(name) => {name.replace(" ", "_")}
        }
    }
}

impl EmojiKind {
    const DELIMITER: &'static str = "[_ -]";

    fn regex_impl() -> regex::Regex {
        let sequences_prefix = vec![
            "Flag",
            "ZWJ",
            "Keycap",
            "Modifier"
        ].iter().join("|");

        let sequences = format!(r"(({sequence_prefix}){delim})?Sequence",
            sequence_prefix = sequences_prefix,
            delim = EmojiKind::DELIMITER
        );

        let postfixes = vec![
            format!("({})", sequences),
            format!("(Modifier({delim}Base)?)", delim = EmojiKind::DELIMITER),
            String::from("Component"),
            String::from("Presentation"),
            format!("extended{delim}pictographic", delim = EmojiKind::DELIMITER)
        ].iter().join("|");

        let advanced_emojis = format!(r"(Emoji{delim})?({postfixes})",
            delim = EmojiKind::DELIMITER,
            postfixes = postfixes
        );

        let regex = format!(r"(?i)(RGI{delim})?(({advanced_emojis})|((Basic{delim})?Emoji))",
            delim = EmojiKind::DELIMITER,
            advanced_emojis = advanced_emojis
        );

        regex::Regex::new(&regex).unwrap()
    }

    pub fn regex() -> &'static regex::Regex {
        lazy_static! {
            static ref REGEX: regex::Regex = EmojiKind::regex_impl();
        }
        &*REGEX
    }
}

#[test]
fn test_kind_regex() {
    let regex: &regex::Regex = EmojiKind::regex();
    let all_kinds = vec![
        EmojiKind::Emoji,
        EmojiKind::EmojiZwjSequence,
        EmojiKind::EmojiSequence,
        EmojiKind::EmojiPresentation,
        EmojiKind::ModifierBase,
        EmojiKind::EmojiComponent,
        EmojiKind::EmojiKeycapSequence,
        EmojiKind::EmojiFlagSequence,
        EmojiKind::EmojiModifierSequence,
    ];
    for kind in all_kinds {
        let kind = kind.to_string();
        assert!(regex.is_match(&kind));
    }
}

impl PartialOrd for EmojiKind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.to_string().partial_cmp(&other.to_string())
    }
}

impl Ord for EmojiKind {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_string().cmp(&other.to_string())
    }
}
