//! Register: how a line should *sound* (specification §11, §14, §15).
//!
//! This is the part that separates a game translation from a correct one. "Are you sure?" has one
//! dictionary reading and several right answers in Vietnamese, and choosing between them is not a
//! vocabulary question:
//!
//! * a wuxia NPC says `Ngươi chắc chứ?`
//! * a modern game's UI says `Bạn có chắc không?`
//! * a shop's confirmation dialog says `Quý khách có chắc chắn không?`
//!
//! Vietnamese has no neutral second person, so the choice cannot be avoided - a translator who
//! ignores it produces text that reads as a machine's. The same problem exists in Japanese and
//! Korean and, more weakly, in the T/V distinction of Russian and the European languages.
//!
//! What this module does is make the choice explicit and then hold the whole game to it. It does
//! not rewrite anyone's wording: it proposes the pronouns a line should use and reports where a
//! line breaks the register the project chose.

use crate::lang::Language;
use crate::translation::Issue;
use serde::{Deserialize, Serialize};

/// Who is speaking, as far as the extractor can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Speaker {
    /// The game talking to the player: menus, errors, tooltips.
    #[default]
    System,
    /// A character talking to the player.
    Npc,
    /// The player's own line.
    Player,
    /// Narration, with no addressee.
    Narrator,
}

/// How the speaker stands towards the listener.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stance {
    /// A superior, an elder, a shopkeeper to a customer.
    Deferential,
    #[default]
    Neutral,
    /// A peer, a companion.
    Familiar,
    /// An enemy, a challenge.
    Hostile,
}

/// The pronouns a register uses.
///
/// Empty strings mean the register does not use a pronoun in that position, which is normal:
/// interface text in Vietnamese usually has no pronoun at all, and inserting one makes it worse.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pronouns {
    pub first_singular: String,
    pub second_singular: String,
    pub first_plural: String,
    pub second_plural: String,
    pub third_male: String,
    pub third_female: String,
}

/// A named way of speaking, chosen per project and applied to every line.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleProfile {
    pub id: String,
    pub language: Language,
    pub description: String,
    /// Pronouns for each speaker and stance, most specific first.
    pub voices: Vec<Voice>,
    /// Wording this register prefers: `(instead_of, use)`.
    #[serde(default)]
    pub prefer: Vec<(String, String)>,
    /// Words that break the register if they appear, with what to say instead.
    #[serde(default)]
    pub avoid: Vec<(String, String)>,
}

/// Pronouns for one speaker in one stance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    pub speaker: Speaker,
    pub stance: Stance,
    pub pronouns: Pronouns,
}

impl StyleProfile {
    /// The pronouns to use for a line, falling back to the profile's neutral voice.
    pub fn pronouns(&self, speaker: Speaker, stance: Stance) -> Pronouns {
        self.voices
            .iter()
            .find(|v| v.speaker == speaker && v.stance == stance)
            .or_else(|| {
                self.voices
                    .iter()
                    .find(|v| v.speaker == speaker && v.stance == Stance::Neutral)
            })
            .or_else(|| self.voices.first())
            .map(|v| v.pronouns.clone())
            .unwrap_or_default()
    }

    /// Reports wording that does not belong in this register.
    ///
    /// Only reports. Substituting `bạn` for `ngươi` inside a finished sentence would leave the
    /// rest of the sentence built around the wrong reading, which is worse than the word itself.
    pub fn check(&self, target: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let lowered = target.to_lowercase();

        for (word, instead) in &self.avoid {
            if contains_word(&lowered, &word.to_lowercase()) {
                issues.push(Issue {
                    code: "register".into(),
                    detail: format!(
                        "{:?} does not suit the {} register; consider {:?}",
                        word, self.id, instead
                    ),
                });
            }
        }
        for (instead_of, use_this) in &self.prefer {
            if contains_word(&lowered, &instead_of.to_lowercase())
                && !contains_word(&lowered, &use_this.to_lowercase())
            {
                issues.push(Issue {
                    code: "wording".into(),
                    detail: format!("this register prefers {use_this:?} to {instead_of:?}"),
                });
            }
        }
        issues
    }
}

/// Whether `word` appears in `text` as a word rather than inside another one.
///
/// Vietnamese words are space-separated and many are short - `ta` sits inside `tay`, `khoáng`,
/// `hoàn tất` - so a substring test would flag half the text.
fn contains_word(text: &str, word: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let needle: Vec<char> = word.chars().collect();
    if needle.is_empty() || needle.len() > chars.len() {
        return false;
    }
    (0..=chars.len() - needle.len()).any(|i| {
        chars[i..i + needle.len()] == needle[..]
            && (i == 0 || !chars[i - 1].is_alphanumeric())
            && (i + needle.len() == chars.len() || !chars[i + needle.len()].is_alphanumeric())
    })
}

fn voice(speaker: Speaker, stance: Stance, pronouns: [&str; 6]) -> Voice {
    Voice {
        speaker,
        stance,
        pronouns: Pronouns {
            first_singular: pronouns[0].into(),
            second_singular: pronouns[1].into(),
            first_plural: pronouns[2].into(),
            second_plural: pronouns[3].into(),
            third_male: pronouns[4].into(),
            third_female: pronouns[5].into(),
        },
    }
}

/// The style profiles shipped with this build.
///
/// A project may define its own; these exist so a new project has something coherent to start
/// from rather than an empty setting that produces mixed register across a whole game.
pub fn builtin_profiles() -> Vec<StyleProfile> {
    vec![
        wuxia_vi(),
        modern_vi(),
        formal_vi(),
        terse_vi(),
        neutral(Language::new("en"), "en-plain", "Plain English."),
        neutral(
            Language::new("zh-Hans"),
            "zh-plain",
            "Plain Simplified Chinese.",
        ),
        neutral(Language::new("th"), "th-plain", "Plain Thai."),
        neutral(Language::new("id"), "id-plain", "Plain Indonesian."),
    ]
}

/// The register most J2ME wuxia and xianxia games want, and the one Vietnamese players expect
/// from them: archaic, distant, no modern pronouns anywhere.
fn wuxia_vi() -> StyleProfile {
    StyleProfile {
        id: "natural-dialogue".into(),
        language: Language::new("vi-VN"),
        description: "Kiếm hiệp / tiên hiệp: ta - ngươi, archaic and distant.".into(),
        voices: vec![
            voice(
                Speaker::Npc,
                Stance::Neutral,
                ["ta", "ngươi", "chúng ta", "các ngươi", "hắn", "nàng"],
            ),
            voice(
                Speaker::Npc,
                Stance::Deferential,
                ["tại hạ", "các hạ", "chúng ta", "chư vị", "hắn", "nàng"],
            ),
            voice(
                Speaker::Npc,
                Stance::Hostile,
                ["ta", "ngươi", "chúng ta", "lũ các ngươi", "hắn", "ả"],
            ),
            voice(
                Speaker::Npc,
                Stance::Familiar,
                ["ta", "ngươi", "chúng ta", "các ngươi", "hắn", "nàng"],
            ),
            voice(
                Speaker::Player,
                Stance::Neutral,
                ["ta", "ngươi", "chúng ta", "các ngươi", "hắn", "nàng"],
            ),
            voice(
                Speaker::Narrator,
                Stance::Neutral,
                ["", "", "", "", "hắn", "nàng"],
            ),
            // Interface text has no speaker and takes no pronoun.
            voice(Speaker::System, Stance::Neutral, ["", "", "", "", "", ""]),
        ],
        prefer: vec![
            ("vũ khí".into(), "binh khí".into()),
            ("kỹ năng".into(), "võ công".into()),
            ("nhiệm vụ phụ".into(), "phụ bản".into()),
        ],
        avoid: vec![
            ("bạn".into(), "ngươi".into()),
            ("tôi".into(), "ta".into()),
            ("mình".into(), "ta".into()),
            ("các bạn".into(), "các ngươi".into()),
            ("ok".into(), "được".into()),
        ],
    }
}

/// Contemporary games: casual mobile, sports, racing, puzzle.
fn modern_vi() -> StyleProfile {
    StyleProfile {
        id: "modern".into(),
        language: Language::new("vi-VN"),
        description: "Contemporary: tôi - bạn, plain and current.".into(),
        voices: vec![
            voice(
                Speaker::Npc,
                Stance::Neutral,
                ["tôi", "bạn", "chúng tôi", "các bạn", "anh ấy", "cô ấy"],
            ),
            voice(
                Speaker::Npc,
                Stance::Familiar,
                ["mình", "cậu", "bọn mình", "các cậu", "cậu ấy", "cô ấy"],
            ),
            voice(
                Speaker::Player,
                Stance::Neutral,
                ["tôi", "bạn", "chúng tôi", "các bạn", "anh ấy", "cô ấy"],
            ),
            voice(
                Speaker::System,
                Stance::Neutral,
                ["", "bạn", "", "các bạn", "", ""],
            ),
            voice(
                Speaker::Narrator,
                Stance::Neutral,
                ["", "", "", "", "anh ấy", "cô ấy"],
            ),
        ],
        prefer: vec![("binh khí".into(), "vũ khí".into())],
        avoid: vec![
            ("ngươi".into(), "bạn".into()),
            ("tại hạ".into(), "tôi".into()),
            ("các hạ".into(), "bạn".into()),
        ],
    }
}

/// Storefronts, payment flows, terms - anywhere the game is a business talking to a customer.
fn formal_vi() -> StyleProfile {
    StyleProfile {
        id: "formal".into(),
        language: Language::new("vi-VN"),
        description: "Formal: quý khách, for shops, payments and notices.".into(),
        voices: vec![
            voice(
                Speaker::System,
                Stance::Neutral,
                ["chúng tôi", "quý khách", "chúng tôi", "quý khách", "", ""],
            ),
            voice(
                Speaker::Npc,
                Stance::Deferential,
                [
                    "chúng tôi",
                    "quý khách",
                    "chúng tôi",
                    "quý khách",
                    "anh ấy",
                    "cô ấy",
                ],
            ),
        ],
        prefer: vec![],
        avoid: vec![
            ("ngươi".into(), "quý khách".into()),
            ("cậu".into(), "quý khách".into()),
            ("mày".into(), "quý khách".into()),
        ],
    }
}

/// Buttons and labels, where a pronoun is a bug.
fn terse_vi() -> StyleProfile {
    StyleProfile {
        id: "terse-ui".into(),
        language: Language::new("vi-VN"),
        description: "Interface labels: no pronouns, imperative, as short as the original.".into(),
        voices: vec![voice(
            Speaker::System,
            Stance::Neutral,
            ["", "", "", "", "", ""],
        )],
        prefer: vec![],
        avoid: vec![
            ("bạn".into(), "bỏ hẳn đại từ".into()),
            ("ngươi".into(), "bỏ hẳn đại từ".into()),
            ("quý khách".into(), "bỏ hẳn đại từ".into()),
        ],
    }
}

/// A profile for a language whose register rules this build does not model.
///
/// Present rather than absent so every language has a profile, and honest about what it is: it
/// checks nothing. A language with a T/V distinction - Russian, German, French - needs the same
/// treatment Vietnamese gets here, and does not have it yet.
fn neutral(language: Language, id: &str, description: &str) -> StyleProfile {
    StyleProfile {
        id: id.into(),
        language,
        description: description.into(),
        voices: Vec::new(),
        prefer: Vec::new(),
        avoid: Vec::new(),
    }
}

/// The profile with this id, if this build ships one.
pub fn builtin(id: &str) -> Option<StyleProfile> {
    builtin_profiles().into_iter().find(|p| p.id == id)
}

/// Profiles that apply to a language.
pub fn profiles_for(language: &Language) -> Vec<StyleProfile> {
    builtin_profiles()
        .into_iter()
        .filter(|p| p.language.same_language_as(language))
        .collect()
}
