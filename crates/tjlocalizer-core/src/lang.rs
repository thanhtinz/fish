//! Language identity and the typographic rules that follow from it.
//!
//! Everything here is about *a* language, never about Vietnamese in particular. The core needs
//! this because the checks that make a translation publishable are not universal: "there is no
//! space before a comma" is true in Vietnamese and English and meaningless in Thai, which has no
//! comma and no spaces between words at all. A length check tuned for Vietnamese would flag every
//! correct Chinese translation, because Chinese says the same thing in a third of the characters.

use serde::{Deserialize, Serialize};

/// A language tag, in the shape of BCP 47: `vi-VN`, `en`, `zh-Hans`, `pt-BR`.
///
/// Stored as written and compared by its parts, so `vi` and `vi-VN` are the same language with
/// different specificity rather than two unrelated strings.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Language(String);

impl Language {
    pub fn new(tag: impl Into<String>) -> Self {
        Language(tag.into().trim().to_string())
    }

    pub fn tag(&self) -> &str {
        &self.0
    }

    /// The primary subtag, lowercased: `vi-VN` gives `vi`.
    pub fn base(&self) -> String {
        self.0.split(['-', '_']).next().unwrap_or("").to_lowercase()
    }

    /// The script subtag when written out: `zh-Hans` gives `Hans`.
    ///
    /// A four-letter subtag is a script; two letters is a region. `zh-Hans` and `zh-TW` are
    /// therefore read differently even though both are two-part tags.
    pub fn script_subtag(&self) -> Option<String> {
        self.0
            .split(['-', '_'])
            .skip(1)
            .find(|part| part.len() == 4 && part.chars().all(|c| c.is_ascii_alphabetic()))
            .map(|part| {
                let mut chars = part.chars();
                let first = chars.next().unwrap().to_ascii_uppercase();
                format!("{first}{}", chars.as_str().to_ascii_lowercase())
            })
    }

    /// The region subtag: `vi-VN` gives `VN`.
    pub fn region(&self) -> Option<String> {
        self.0
            .split(['-', '_'])
            .skip(1)
            .find(|part| part.len() == 2 && part.chars().all(|c| c.is_ascii_alphabetic()))
            .map(|part| part.to_ascii_uppercase())
    }

    /// True when the two tags name the same language, whatever their region or script.
    pub fn same_language_as(&self, other: &Language) -> bool {
        self.base() == other.base()
    }

    /// How the language is written, which is what the typographic rules key off.
    pub fn script(&self) -> Script {
        if let Some(subtag) = self.script_subtag() {
            return match subtag.as_str() {
                "Hans" | "Hant" => Script::Han,
                "Latn" => Script::Latin,
                "Cyrl" => Script::Cyrillic,
                "Thai" => Script::Thai,
                "Jpan" => Script::Japanese,
                "Kore" | "Hang" => Script::Korean,
                "Arab" => Script::Arabic,
                _ => Script::Latin,
            };
        }
        match self.base().as_str() {
            "zh" | "yue" => Script::Han,
            "ja" => Script::Japanese,
            "ko" => Script::Korean,
            "th" => Script::Thai,
            "ru" | "uk" | "bg" | "sr" | "be" | "kk" => Script::Cyrillic,
            "ar" | "fa" | "ur" => Script::Arabic,
            _ => Script::Latin,
        }
    }

    /// A name for the language in English, for interfaces. Falls back to the tag itself, which is
    /// better than an empty string and honest about not knowing.
    pub fn display_name(&self) -> String {
        let name = match self.base().as_str() {
            "vi" => "Vietnamese",
            "en" => "English",
            "zh" => match self.script_subtag().as_deref() {
                Some("Hant") => "Chinese (Traditional)",
                Some("Hans") => "Chinese (Simplified)",
                _ => match self.region().as_deref() {
                    Some("TW") | Some("HK") | Some("MO") => "Chinese (Traditional)",
                    _ => "Chinese",
                },
            },
            "ja" => "Japanese",
            "ko" => "Korean",
            "ru" => "Russian",
            "th" => "Thai",
            "id" => "Indonesian",
            "ms" => "Malay",
            "es" => "Spanish",
            "pt" => "Portuguese",
            "fr" => "French",
            "de" => "German",
            "tr" => "Turkish",
            "hi" => "Hindi",
            "ar" => "Arabic",
            "fil" | "tl" => "Filipino",
            "km" => "Khmer",
            "lo" => "Lao",
            "my" => "Burmese",
            _ => return self.0.clone(),
        };
        name.to_string()
    }

    /// The typographic and quality rules for this language.
    pub fn profile(&self) -> LanguageProfile {
        LanguageProfile::for_language(self)
    }
}

impl Default for Language {
    /// `und` - the tag for "undetermined". A default of English or Vietnamese would be a guess
    /// presented as a fact, and a wrong source language silently disables every dictionary.
    fn default() -> Self {
        Language::new("und")
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Language {
    fn from(tag: &str) -> Self {
        Language::new(tag)
    }
}

/// The writing system, which decides how text may be measured and spaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Script {
    Latin,
    Han,
    Japanese,
    Korean,
    Thai,
    Cyrillic,
    Arabic,
}

impl Script {
    /// Whether words are separated by spaces. Han, Japanese and Thai run text together, so any
    /// rule that counts or splits on spaces is wrong for them.
    pub fn uses_spaces_between_words(self) -> bool {
        !matches!(self, Script::Han | Script::Japanese | Script::Thai)
    }

    /// Roughly how many characters this script needs to say what one English character says.
    ///
    /// Used to set a sane length budget: a Chinese translation of an English label is normally
    /// much shorter, and a Thai or Vietnamese one somewhat longer. Without this the length check
    /// either never fires or fires on everything, depending on which language it was tuned for.
    pub fn density(self) -> f32 {
        match self {
            Script::Han => 0.4,
            Script::Japanese => 0.6,
            Script::Korean => 0.7,
            Script::Latin => 1.0,
            Script::Cyrillic => 1.1,
            Script::Thai => 1.15,
            Script::Arabic => 1.0,
        }
    }
}

/// What "well formed" means in one language.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageProfile {
    pub language: Language,
    pub script: Script,
    /// Punctuation that takes no space before and one space after. Empty where the script does
    /// not space its punctuation at all.
    pub tight_punctuation: Vec<char>,
    /// Multiplier on the source length past which a translation is suspiciously long, before the
    /// script's own density is taken into account.
    pub expansion_limit: f32,
    /// Whether a full stop is expected to end a sentence. Chinese and Japanese use their own
    /// terminators, and Thai uses none.
    pub uses_ascii_sentence_end: bool,
}

impl LanguageProfile {
    pub fn for_language(language: &Language) -> Self {
        let script = language.script();
        let tight_punctuation = match script {
            // Han and Japanese punctuation is full-width and carries its own spacing.
            Script::Han | Script::Japanese => Vec::new(),
            // Thai writes without spaces between words and has no comma or full stop of its own.
            Script::Thai => Vec::new(),
            _ => vec![',', '.', '!', '?', ';', ':'],
        };
        LanguageProfile {
            language: language.clone(),
            script,
            tight_punctuation,
            expansion_limit: 3.0,
            uses_ascii_sentence_end: matches!(
                script,
                Script::Latin | Script::Cyrillic | Script::Thai
            ),
        }
    }

    /// The character count past which a translation is worth questioning.
    pub fn length_budget(&self, source_len: usize, source: &Language) -> usize {
        let ratio = self.script.density() / source.script().density();
        ((source_len as f32) * ratio * self.expansion_limit).ceil() as usize
    }
}

/// The languages this build ships rules and dictionary data for, as source or target.
///
/// Not a limit: any tag may be used, and an unknown one simply falls back to the rules its script
/// implies. This list exists so an interface can offer something rather than an empty box.
pub fn known_languages() -> Vec<Language> {
    [
        "vi-VN", "en", "zh-Hans", "zh-Hant", "th", "id", "ja", "ko", "ru", "ms", "es", "pt-BR",
        "fr", "de", "tr", "ar", "hi", "fil", "km", "lo", "my",
    ]
    .iter()
    .map(|t| Language::new(*t))
    .collect()
}
