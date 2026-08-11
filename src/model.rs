use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    Emoji,
    Kaomoji,
    Ascii,
    Snippet,
    Symbol,
}

impl ItemKind {
    pub const ALL: [Self; 5] = [
        Self::Emoji,
        Self::Kaomoji,
        Self::Ascii,
        Self::Snippet,
        Self::Symbol,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Emoji => "Emoji",
            Self::Kaomoji => "Kaomoji",
            Self::Ascii => "ASCII",
            Self::Snippet => "Text",
            Self::Symbol => "Symbols",
        }
    }

    pub fn customizable(self) -> bool {
        matches!(self, Self::Kaomoji | Self::Ascii | Self::Snippet)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PickerItem {
    pub kind: ItemKind,
    pub title: &'static str,
    pub content: &'static str,
    pub keywords: &'static str,
}
