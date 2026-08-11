#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemKind {
    Emoji,
    Kaomoji,
    Ascii,
    Snippet,
    Symbol,
}

impl ItemKind {
    pub const ALL: [Option<Self>; 6] = [
        None,
        Some(Self::Emoji),
        Some(Self::Kaomoji),
        Some(Self::Ascii),
        Some(Self::Snippet),
        Some(Self::Symbol),
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
}

#[derive(Clone, Copy, Debug)]
pub struct PickerItem {
    pub kind: ItemKind,
    pub title: &'static str,
    pub content: &'static str,
    pub keywords: &'static str,
}
