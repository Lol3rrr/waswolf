use std::fmt::Display;

use serenity::model::channel::ReactionType;

/// The Reactions used by the Bot
#[derive(Debug, PartialEq, Clone)]
pub enum Reactions {
    Entry,
    ModEntry,
    Confirm,
    Stop,
    NextPage,
    PreviousPage,
    Custom(String),
}

impl Reactions {
    pub fn to_str(&self) -> &str {
        match self {
            Self::Entry => "✅",
            Self::ModEntry => "🇲",
            Self::Confirm => "🆗",
            Self::Stop => "🛑",
            Self::NextPage => "👉",
            Self::PreviousPage => "👈",
            Self::Custom(val) => val,
        }
    }
}

impl Display for Reactions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

impl From<Reactions> for ReactionType {
    fn from(other: Reactions) -> Self {
        ReactionType::Unicode(other.to_str().to_owned())
    }
}
impl From<&Reactions> for ReactionType {
    fn from(other: &Reactions) -> Self {
        ReactionType::Unicode(other.to_str().to_owned())
    }
}

impl PartialEq<ReactionType> for Reactions {
    fn eq(&self, other: &ReactionType) -> bool {
        other.unicode_eq(self.to_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equals() {
        assert!(Reactions::Entry == ReactionType::from('✅'));
    }
}
