//! What a line is for, and who says it, read from the lines around it (§10, §5, §15).
//!
//! `graph::classify` looks at one string on its own, and one string on its own is often not
//! enough. `Yes` is two characters and no punctuation, so it is a button - and it is also half
//! the answers in a conversation. `Iron Sword` is a short noun phrase, which is an interface
//! label and an item name and a quest objective. Nothing about those strings settles it.
//!
//! What settles it is the company they keep. A string sitting between two lines of dialogue in
//! the same class is dialogue; a key under `menu.` beside eleven other keys that are all buttons
//! is a button; a line beginning `Blacksmith:` is a character speaking, and the character has a
//! name that appears in nine other lines. None of that is knowledge about a game - it is the
//! shape of how games are written down - so it belongs here rather than in a per-game rule.
//!
//! Two things this module refuses to do. It never overrules a string that settled its own case:
//! a path, a format string, a whole sentence. What it may reconsider is the reading a short
//! string got by being short - `Yes` was called a button because it is two characters long, and
//! that is a description of its length rather than a finding about its use. And it never decides
//! a register on its own. What it produces is a set of readings with the evidence for each one,
//! offered to a person; §14's whole argument is that the voice a game speaks in is a decision,
//! and a decision inferred from a colon in a string is not a decision.

use crate::graph::{ContentGraph, ContextType, TextSource};
use crate::register::{Speaker, Stance};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What was worked out about one node, and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub node: String,
    /// A context the surroundings suggest, where the node had none of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<ContextType>,
    /// Who says it, where the line says so.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<Speaker>,
    /// The character speaking, by the name the game writes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
    /// Where the line's own text ends, when a speaker's name was found in front of it.
    ///
    /// A translator must not translate `Blacksmith:` as part of the line - the game prints that
    /// prefix itself in half the engines that use it - and cannot see where it ends without being
    /// told.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spoken_text: Option<String>,
    /// One line each, in the tool's English, for a person checking the reading.
    pub why: Vec<String>,
}

/// A character the game names, and everything found out about them.
///
/// The specification's §5 asks for a character and relationship layer. This is what can honestly
/// be filled into one from text alone: who is named, how often, where, and beside whom. Anything
/// more - who is whose brother, who outranks whom - is in the game's story and not in its strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    /// The name as the game writes it.
    pub name: String,
    /// How many lines are attributed to them.
    pub lines: usize,
    /// The classes and resources their lines are in.
    pub appears_in: Vec<String>,
    /// Characters named in the same file. Not a relationship - a co-occurrence, which is the most
    /// a reader of strings can say, and enough to group a cast into scenes.
    pub beside: Vec<String>,
    /// A stance their lines suggest, with what suggested it. A proposal, never applied: the
    /// register a game speaks in is a decision, and §14 does not let this module make it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_stance: Option<StanceHint>,
}

/// A stance one character's lines lean towards, and the words that leaned that way.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StanceHint {
    pub stance: Stance,
    /// The words found, verbatim, so a person can judge the reading rather than trust it.
    pub because: Vec<String>,
}

/// Everything one pass over a graph worked out.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Inference {
    pub readings: Vec<Reading>,
    pub cast: Vec<Character>,
}

impl Inference {
    pub fn reading(&self, node: &str) -> Option<&Reading> {
        self.readings.iter().find(|r| r.node == node)
    }

    /// The voice a node should be translated in, as far as anything here can tell.
    ///
    /// Falls back to the game talking to the player, which is what interface text is and what
    /// most of a game's strings are.
    pub fn voice(&self, node: &str) -> (Speaker, Stance) {
        let Some(reading) = self.reading(node) else {
            return (Speaker::default(), Stance::default());
        };
        let speaker = reading.speaker.unwrap_or_default();
        let stance = reading
            .character
            .as_ref()
            .and_then(|name| self.cast.iter().find(|c| &c.name == name))
            .and_then(|c| c.suggested_stance.as_ref())
            .map(|hint| hint.stance)
            .unwrap_or_default();
        (speaker, stance)
    }
}

/// Reads a whole graph for what its nodes are, and who speaks them.
pub fn infer(graph: &ContentGraph) -> Inference {
    let mut readings: BTreeMap<String, Reading> = BTreeMap::new();

    speakers(graph, &mut readings);
    by_key_prefix(graph, &mut readings);
    by_neighbour(graph, &mut readings);

    let cast = cast(graph, &readings);
    Inference {
        readings: readings.into_values().collect(),
        cast,
    }
}

/// Whether a string's own shape settled what it is.
///
/// A path, a format string, a sentence, a paragraph: those are findings about the text. `Ui` on a
/// short string is not - it says "this is short", which is true of a button, of an item name and
/// of half the replies in a conversation. Those two, and only those two, are open to what the
/// surroundings say.
fn settled(context: ContextType) -> bool {
    !matches!(context, ContextType::Unknown | ContextType::Ui)
}

fn entry<'a>(readings: &'a mut BTreeMap<String, Reading>, node: &str) -> &'a mut Reading {
    readings.entry(node.to_string()).or_insert_with(|| Reading {
        node: node.to_string(),
        context: None,
        speaker: None,
        character: None,
        spoken_text: None,
        why: Vec::new(),
    })
}

// -------------------------------------------------------------------------------------------
// Who is speaking
// -------------------------------------------------------------------------------------------

/// Finds `Name: line`, which is how a very large share of games write down speech.
///
/// Deliberately strict. A colon appears in `HP: 20`, in `Time: 3:00`, in `Error: file not found`,
/// and in a URL, and reading any of those as a character called `HP` would put a fictional cast
/// in front of a translator. So the part before the colon must look like a name - short, no
/// digits, no punctuation, starting with a capital - and the part after it must look like
/// something a person would say.
pub fn speaker_prefix(text: &str) -> Option<(&str, &str)> {
    let (name, rest) = text.split_once(':')?;
    let name = name.trim();
    let rest = rest.trim_start();

    if name.is_empty() || rest.is_empty() {
        return None;
    }
    if name.chars().count() > 24 || name.split_whitespace().count() > 3 {
        return None;
    }
    if !name.chars().next()?.is_uppercase() {
        return None;
    }
    if name
        .chars()
        .any(|c| c.is_ascii_digit() || (!c.is_alphanumeric() && !" '-_.".contains(c)))
    {
        return None;
    }
    // A name followed by one word is a label with a value in it - `Level: 20`, `Gold: 5` - far
    // more often than it is a line of speech.
    if rest.split_whitespace().count() < 2 {
        return None;
    }
    // A placeholder after the colon is a format string: `Score: %d`, not somebody talking.
    if !crate::graph::find_placeholders(rest).is_empty() {
        return None;
    }
    Some((name, rest))
}

fn speakers(graph: &ContentGraph, readings: &mut BTreeMap<String, Reading>) {
    for node in &graph.nodes {
        if !node.context.is_translatable() {
            continue;
        }
        let Some((name, said)) = speaker_prefix(&node.source_text) else {
            continue;
        };
        let reading = entry(readings, &node.id);
        reading.speaker = Some(Speaker::Npc);
        reading.character = Some(name.to_string());
        reading.spoken_text = Some(said.to_string());
        reading
            .why
            .push(format!("the line begins {name:?} and a colon"));
        if !settled(node.context) {
            reading.context = Some(ContextType::Dialogue);
            reading
                .why
                .push("a named speaker makes it dialogue rather than a label".into());
        }
    }
}

// -------------------------------------------------------------------------------------------
// What a line is for
// -------------------------------------------------------------------------------------------

/// The part of a resource key before its last separator: `menu.audio.volume` groups under
/// `menu.audio`.
fn key_prefix(key: &str) -> Option<&str> {
    let cut = key.rfind(['.', '/', '_', ':'])?;
    let prefix = &key[..cut];
    (!prefix.is_empty()).then_some(prefix)
}

/// What a group of keys is called, where the name is one games agree on.
///
/// A naming convention is not knowledge about a game - `menu`, `dialog`, `quest` and `item` mean
/// the same thing across every game that uses them, which is why they are conventions - so this
/// list is general in the way §2 requires. It only ever fires on a group of keys, never on one:
/// a single key called `item` is a word, and twelve keys under `item.` are a section.
fn prefix_meaning(prefix: &str) -> Option<ContextType> {
    // Read right to left: `quest.iron` is a quest, and in `ui.quest.title` the nearer word wins
    // because a key is written from the general to the particular.
    prefix
        .rsplit(['.', '/', '_', ':'])
        .find_map(|segment| meaning_of(&segment.to_lowercase()))
}

fn meaning_of(word: &str) -> Option<ContextType> {
    let meaning = match word {
        "menu" | "ui" | "button" | "buttons" | "label" | "labels" | "screen" | "settings"
        | "options" => ContextType::Ui,
        "dialog" | "dialogue" | "talk" | "npc" | "chat" | "conversation" | "speech" => {
            ContextType::Dialogue
        }
        "quest" | "quests" | "mission" | "missions" | "task" | "tasks" => ContextType::Quest,
        "item" | "items" | "equip" | "equipment" | "weapon" | "weapons" | "armor" | "armour" => {
            ContextType::Item
        }
        "skill" | "skills" | "spell" | "spells" | "ability" | "abilities" | "magic" => {
            ContextType::Skill
        }
        "error" | "errors" | "system" | "msg" | "message" | "messages" | "notice" => {
            ContextType::System
        }
        "tutorial" | "tutorials" | "help" | "hint" | "hints" => ContextType::Tutorial,
        "story" | "lore" | "intro" | "cutscene" => ContextType::Story,
        _ => return None,
    };
    Some(meaning)
}

fn by_key_prefix(graph: &ContentGraph, readings: &mut BTreeMap<String, Reading>) {
    // Group every keyed node under the prefix of its key.
    let mut groups: BTreeMap<(&str, &str), Vec<&crate::graph::TextNode>> = BTreeMap::new();
    for node in &graph.nodes {
        let TextSource::ResourceProperty { resource, key } = &node.source else {
            continue;
        };
        if let Some(prefix) = key_prefix(key) {
            groups
                .entry((resource.as_str(), prefix))
                .or_default()
                .push(node);
        }
    }

    for ((resource, prefix), nodes) in groups {
        if nodes.len() < 3 {
            // Two keys are not a section. A rule that fired on them would relabel a pair of
            // strings from their file name, which is guessing dressed as inference.
            continue;
        }

        // What the group is called, and what the group's own confidently-classified members are.
        let named = prefix_meaning(prefix);
        // Keyed by the context's name rather than the context, because a content type is not
        // ordered and does not need to be for this.
        let mut counts: BTreeMap<&str, (ContextType, usize)> = BTreeMap::new();
        for node in &nodes {
            if settled(node.context) && node.context.is_translatable() {
                let slot = counts
                    .entry(node.context.key())
                    .or_insert((node.context, 0));
                slot.1 += 1;
            }
        }
        let majority = counts
            .values()
            .copied()
            .max_by_key(|(_, n)| *n)
            .filter(|(_, n)| *n * 2 > nodes.len())
            .map(|(context, _)| context);

        for node in &nodes {
            if settled(node.context) {
                continue;
            }
            let (context, why) = match (named, majority) {
                (Some(named), _) => (
                    named,
                    format!(
                        "it is under {prefix:?} in {resource}, which names a {}",
                        named.key()
                    ),
                ),
                (None, Some(majority)) => (
                    majority,
                    format!(
                        "the other keys under {prefix:?} in {resource} are mostly {}",
                        majority.key()
                    ),
                ),
                (None, None) => continue,
            };
            // Saying a short string in a menu is interface text tells nobody anything: it is
            // what the graph already called it.
            if context == node.context {
                continue;
            }
            let reading = entry(readings, &node.id);
            if reading.context.is_none() {
                reading.context = Some(context);
                reading.why.push(why);
            }
        }
    }
}

/// A string with nothing to say for itself, between two that agree.
///
/// Strings in a class constant pool arrive in the order the compiler wrote them, which follows
/// the order they appear in the source: a screen's labels are together, a conversation's lines
/// are together. That ordering is weak evidence and it is real evidence, and it is the only
/// evidence there is for a bare `Yes`.
fn by_neighbour(graph: &ContentGraph, readings: &mut BTreeMap<String, Reading>) {
    let mut by_class: BTreeMap<&str, Vec<&crate::graph::TextNode>> = BTreeMap::new();
    for node in &graph.nodes {
        if let TextSource::ClassConstant { class, .. } = &node.source {
            by_class.entry(class.as_str()).or_default().push(node);
        }
    }

    for (class, nodes) in by_class {
        for (i, node) in nodes.iter().enumerate() {
            if settled(node.context) {
                continue;
            }
            let before = nodes[..i]
                .iter()
                .rev()
                .find(|n| settled(n.context) && n.context.is_translatable());
            let after = nodes[i + 1..]
                .iter()
                .find(|n| settled(n.context) && n.context.is_translatable());

            // Both sides, and agreeing. One neighbour is a coincidence; two that say the same
            // thing are a section of a file.
            let (Some(before), Some(after)) = (before, after) else {
                continue;
            };
            if before.context != after.context {
                continue;
            }
            let reading = entry(readings, &node.id);
            if reading.context.is_none() {
                reading.context = Some(before.context);
                reading.why.push(format!(
                    "the strings either side of it in {class} are both {}",
                    before.context.key()
                ));
            }
        }
    }
}

// -------------------------------------------------------------------------------------------
// The cast
// -------------------------------------------------------------------------------------------

/// Words that lean a line one way or another, and how far.
///
/// English, because the source text of a game being localized into Vietnamese is nearly always
/// English, and because a list that pretended to cover every source language would cover none.
/// Weak on purpose: these appear in the evidence a person reads, not in a decision.
const DEFERENTIAL: [&str; 10] = [
    "please",
    "sir",
    "madam",
    "lord",
    "lady",
    "master",
    "honoured",
    "honored",
    "majesty",
    "excellency",
];
const HOSTILE: [&str; 10] = [
    "fool", "coward", "die", "kill you", "worm", "scum", "pathetic", "insolent", "traitor",
    "beg for",
];
const FAMILIAR: [&str; 8] = [
    "hey",
    "buddy",
    "pal",
    "friend",
    "come on",
    "let's",
    "mate",
    "look here",
];

fn cast(graph: &ContentGraph, readings: &BTreeMap<String, Reading>) -> Vec<Character> {
    let mut by_name: BTreeMap<String, Character> = BTreeMap::new();
    // Which characters were found in which file, so co-occurrence can be worked out after.
    let mut per_file: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for node in &graph.nodes {
        let Some(reading) = readings.get(&node.id) else {
            continue;
        };
        let Some(name) = &reading.character else {
            continue;
        };
        let where_from = match &node.source {
            TextSource::ClassConstant { class, .. } => class.clone(),
            TextSource::ResourceProperty { resource, .. }
            | TextSource::ResourceLine { resource, .. } => resource.clone(),
        };

        let character = by_name.entry(name.clone()).or_insert_with(|| Character {
            name: name.clone(),
            lines: 0,
            appears_in: Vec::new(),
            beside: Vec::new(),
            suggested_stance: None,
        });
        character.lines += 1;
        if !character.appears_in.contains(&where_from) {
            character.appears_in.push(where_from.clone());
        }

        let said = reading
            .spoken_text
            .clone()
            .unwrap_or_else(|| node.source_text.clone());
        stance_words(&said, character);

        let names = per_file.entry(where_from).or_default();
        if !names.contains(name) {
            names.push(name.clone());
        }
    }

    for names in per_file.values() {
        for name in names {
            let Some(character) = by_name.get_mut(name) else {
                continue;
            };
            for other in names {
                if other != name && !character.beside.contains(other) {
                    character.beside.push(other.clone());
                }
            }
        }
    }

    by_name.into_values().collect()
}

/// Adds whatever one line says about how its speaker stands towards the listener.
fn stance_words(line: &str, character: &mut Character) {
    let lower = line.to_lowercase();
    let mut found: Vec<(Stance, &str)> = Vec::new();
    for word in DEFERENTIAL {
        if lower.contains(word) {
            found.push((Stance::Deferential, word));
        }
    }
    for word in HOSTILE {
        if lower.contains(word) {
            found.push((Stance::Hostile, word));
        }
    }
    for word in FAMILIAR {
        if lower.contains(word) {
            found.push((Stance::Familiar, word));
        }
    }
    if found.is_empty() {
        return;
    }

    let hint = character.suggested_stance.get_or_insert(StanceHint {
        stance: found[0].0,
        because: Vec::new(),
    });
    for (_, word) in &found {
        if !hint.because.iter().any(|w| w == word) {
            hint.because.push((*word).to_string());
        }
    }

    // Whichever leaning has the most words behind it across everything this character says. A
    // character who says "please" once and "coward" four times is hostile, and one who says both
    // once is a character a person needs to look at rather than a stance this can settle.
    let mut counts: BTreeMap<&str, (Stance, usize)> = BTreeMap::new();
    for word in &hint.because {
        let stance = if DEFERENTIAL.contains(&word.as_str()) {
            Stance::Deferential
        } else if HOSTILE.contains(&word.as_str()) {
            Stance::Hostile
        } else {
            Stance::Familiar
        };
        let key = match stance {
            Stance::Deferential => "deferential",
            Stance::Hostile => "hostile",
            Stance::Familiar => "familiar",
            Stance::Neutral => "neutral",
        };
        let slot = counts.entry(key).or_insert((stance, 0));
        slot.1 += 1;
    }
    if let Some((stance, _)) = counts.values().copied().max_by_key(|(_, n)| *n) {
        hint.stance = stance;
    }
}
