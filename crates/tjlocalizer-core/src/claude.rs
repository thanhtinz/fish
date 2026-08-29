//! Asking Claude the questions the mechanical checks cannot answer.
//!
//! Everything else in this crate decides mechanically: `writeback::plan` reads magic bytes,
//! `assets::hints` counts colours and bands of ink, `package::survey` works from a list of formats
//! somebody wrote down. Mechanical is right for the decisions that have to be *correct* - a wrong
//! guess about whether a file can be written back destroys it. But mechanical runs out exactly
//! where the work is hardest: a Steam folder with forty thousand files in it, or a format nobody
//! has written a reader for.
//!
//! That is what this is for, and it is also precisely where a model must not be allowed to decide.
//! So three rules hold throughout, and each has a test:
//!
//! **Off by default, and off means nothing is sent.** The same guarantee the translation engine
//! makes, enforced the same way - a test counts how many times the transport was called.
//!
//! **What goes out is bounded and visible.** A survey sends file *names, sizes and detected
//! formats*. It does not send file contents. A sample of one file's bytes goes only when somebody
//! asks about that one file. This is a hard boundary, not a default that a checkbox can widen.
//!
//! **Nothing that comes back becomes a fact.** A survey's answers are suggestions shown in their
//! own section with their reasons, never merged into what `package::survey` established and never
//! consulted by `writeback::plan`. A review's answers are notes on a row, never edits. This is the
//! same rule as `Proposal::is_approvable()` returning false: what a machine proposes, a person
//! decides.

use crate::jar::Archive;
use crate::provider::HttpCall;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Where the messages endpoint lives.
pub const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";

/// The version of the wire format this build speaks.
pub const API_VERSION: &str = "2023-06-01";

/// What is said when the engine is off. A sentence rather than a silence: somebody who pressed a
/// button deserves to know why nothing happened.
pub const OFF: &str = "the analysis engine is off, so nothing was sent";

/// The default model.
///
/// A scan of one package is a handful of calls; a scan of a Steam folder is many more. The model
/// is configurable for that reason, and this is the default rather than the only choice.
pub const DEFAULT_MODEL: &str = "claude-opus-5";

/// How the caller has set this up.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Nothing is sent while this is false. It starts false.
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_model() -> String {
    DEFAULT_MODEL.to_string()
}

fn default_timeout() -> u64 {
    120
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            enabled: false,
            model: default_model(),
            timeout_seconds: default_timeout(),
        }
    }
}

/// What is known about one file without opening it.
///
/// This is the whole of what a survey sends per file. Writing it as a type rather than assembling
/// JSON at the call site is the point: the boundary is visible in one place, and a test can assert
/// that nothing else crosses it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileFact {
    pub path: String,
    pub size: u64,
    /// What the mechanical check made of it, so the model is told what is already known.
    pub detected: String,
    /// Whether this build can already read it. A file already read needs no opinion.
    pub readable: bool,
}

/// One file's worth of the model's opinion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileVerdict {
    pub path: String,
    /// Whether it looks like it holds text a player would read.
    pub holds_text: bool,
    /// Why, in a sentence somebody can check against the file.
    pub why: String,
    /// 0.0 to 1.0, the model's own reading of how sure it is.
    pub confidence: f32,
}

/// What a survey produced, kept apart from anything mechanical.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Survey {
    pub verdicts: Vec<FileVerdict>,
    /// The model this came from, so a stale result can be told apart from a fresh one.
    #[serde(default)]
    pub model: String,
}

/// One unknown file, with a small piece of it.
#[derive(Debug, Clone)]
pub struct Sample {
    pub path: String,
    pub size: u64,
    /// The opening bytes, hex-encoded, and however much of it reads as text.
    pub head_hex: String,
    pub head_text: String,
}

impl Sample {
    /// How much of a file is taken. Small on purpose: enough to recognise a format's header and
    /// its first strings, not enough to be a copy of somebody's game.
    pub const BYTES: usize = 2048;

    pub fn of(path: &str, data: &[u8]) -> Self {
        let head = &data[..data.len().min(Self::BYTES)];
        Sample {
            path: path.to_string(),
            size: data.len() as u64,
            head_hex: head.iter().map(|b| format!("{b:02x}")).collect(),
            head_text: head
                .iter()
                .map(|b| {
                    if b.is_ascii_graphic() || *b == b' ' {
                        *b as char
                    } else {
                        '.'
                    }
                })
                .collect(),
        }
    }
}

/// What the model made of one file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    /// What format it appears to be, or "unknown".
    pub format: String,
    /// Where the text sits inside it, in words.
    pub where_text_is: String,
    /// How a translation would be addressed back to a place in the file, if it could be.
    pub addressing: String,
    /// What would make this wrong. Asked for explicitly, because a confident wrong answer about a
    /// binary format is worse than no answer.
    pub caveat: String,
}

/// One thing the model noticed about one translated line.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewNote {
    pub node_id: String,
    /// "register", "terminology", "context", "placeholder", "other".
    pub kind: String,
    pub detail: String,
    /// What it suggests instead. A suggestion, never applied.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub suggestion: String,
}

/// One line put up for review.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewLine {
    pub node_id: String,
    pub context: String,
    pub source: String,
    pub target: String,
}

/// How many files go in one call.
///
/// A game folder has tens of thousands of files and one call cannot hold them all. The number is
/// small enough that a failed batch loses little and large enough that a package is a handful of
/// calls, not hundreds.
pub const BATCH: usize = 200;

/// The whole of what a scan of this package would send.
///
/// Public, and returning a list of typed facts rather than a request body, because this is what a
/// person about to send data actually needs to see: the names. Assembling it here means one test
/// can hold the boundary - what this returns is what goes, and nothing reads an entry's bytes for
/// anything but the mechanical verdict that is already known.
pub fn facts(archive: &Archive) -> Vec<FileFact> {
    archive
        .entries()
        .iter()
        .filter(|entry| !entry.is_class() && !crate::graph::is_archive_metadata(&entry.name))
        .map(|entry| {
            let (detected, readable) = match crate::writeback::plan(&entry.name, &entry.data) {
                crate::writeback::Plan::Text { format, .. } => (format.name().to_string(), true),
                crate::writeback::Plan::Binary(binary) => (binary.name().to_string(), true),
                crate::writeback::Plan::ReadOnly { reason } => (reason, false),
            };
            FileFact {
                path: entry.name.clone(),
                size: entry.data.len() as u64,
                detected,
                readable,
            }
        })
        .collect()
}

/// Builds the calls and reads the replies. Holds no key and does no I/O.
///
/// Separate from `HttpProvider` because the shapes have nothing in common: that one sends a string
/// and expects a string back, this one sends a structured question and expects a structured
/// answer. What they do share is `HttpCall` and the transport behind it, which is why the tested
/// fake-network seam works here unchanged.
pub struct Analyst<'a> {
    pub settings: Settings,
    /// How a call actually reaches the network. Replaced in tests, exactly as the translation
    /// engine's is, so nothing in this module has ever talked to a real service under test.
    transport: Box<dyn Fn(HttpCall) -> Result<String, String> + 'a>,
}

impl<'a> Analyst<'a> {
    pub fn new(settings: Settings) -> Self {
        Analyst {
            settings,
            transport: Box::new(crate::provider::send),
        }
    }

    /// Replaces the transport. For tests, which point it at a local server rather than a service.
    pub fn with_transport(
        mut self,
        transport: impl Fn(HttpCall) -> Result<String, String> + 'a,
    ) -> Self {
        self.transport = Box::new(transport);
        self
    }

    /// Sends one call, or refuses to.
    ///
    /// The one place in this module that reaches the network, so the guarantee holds in one
    /// place: while `enabled` is false nothing is sent, and the transport is not so much as
    /// touched. A test counts the calls.
    fn send(&self, call: HttpCall) -> Result<String, String> {
        if !self.settings.enabled {
            return Err(OFF.to_string());
        }
        (self.transport)(call)
    }

    /// Asks which files hold text. Sends names, sizes and formats; never contents.
    pub fn survey(&self, api_key: &str, files: &[FileFact]) -> Result<Survey, String> {
        let reply = self.send(self.survey_call(api_key, files))?;
        read_survey(&reply, &self.settings.model)
    }

    /// The same, in batches, for a package with more files than fit in one call.
    ///
    /// A batch that fails does not lose the ones already answered: the failure is returned
    /// alongside what was learned, because throwing away nine good answers over a tenth bad one
    /// is not a kindness.
    pub fn survey_all(&self, api_key: &str, files: &[FileFact]) -> (Survey, Vec<String>) {
        let mut survey = Survey {
            verdicts: Vec::new(),
            model: self.settings.model.clone(),
        };
        let mut trouble = Vec::new();
        for batch in files.chunks(BATCH) {
            match self.survey(api_key, batch) {
                Ok(part) => survey.verdicts.extend(part.verdicts),
                Err(why) => trouble.push(why),
            }
        }
        (survey, trouble)
    }

    /// Asks what one file is. The only path that sends any of a file's bytes, and only this
    /// file's, and only [`Sample::BYTES`] of them.
    pub fn inspect(&self, api_key: &str, sample: &Sample) -> Result<Inspection, String> {
        let reply = self.send(self.inspect_call(api_key, sample))?;
        read_inspection(&reply)
    }

    /// Asks what is wrong with approved translations. Sends the game's own text, so it sits
    /// behind its own deliberate action rather than running with a scan.
    pub fn review(
        &self,
        api_key: &str,
        lines: &[ReviewLine],
        register: Option<&str>,
        glossary: &[(String, String)],
    ) -> Result<Vec<ReviewNote>, String> {
        let reply = self.send(self.review_call(api_key, lines, register, glossary))?;
        read_review(&reply)
    }

    /// Counts what a call would cost before it is made.
    pub fn count(&self, api_key: &str, call: &HttpCall) -> Result<u64, String> {
        let reply = self.send(self.count_call(api_key, call))?;
        read_count(&reply)
    }

    fn call(&self, api_key: &str, body: serde_json::Value) -> HttpCall {
        self.call_to(ENDPOINT, api_key, body.to_string())
    }

    fn call_to(&self, url: &str, api_key: &str, body: String) -> HttpCall {
        HttpCall {
            url: url.to_string(),
            headers: vec![
                ("Content-Type".into(), "application/json".into()),
                ("x-api-key".into(), api_key.to_string()),
                ("anthropic-version".into(), API_VERSION.to_string()),
            ],
            body,
            timeout: Duration::from_secs(self.settings.timeout_seconds),
        }
    }

    /// The request that asks which files hold text.
    ///
    /// Public so the interface can show it before anything is sent. What matters to a person about
    /// to send data is not the JSON but the list of names in it, and both are visible here.
    pub fn survey_call(&self, api_key: &str, files: &[FileFact]) -> HttpCall {
        let tool = serde_json::json!({
            "name": "report_files",
            "description": "Report which files hold text a player would read.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" },
                                "holdsText": { "type": "boolean" },
                                "why": {
                                    "type": "string",
                                    "description": "One sentence a person can check against the file."
                                },
                                "confidence": { "type": "number" }
                            },
                            "required": ["path", "holdsText", "why", "confidence"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["files"],
                "additionalProperties": false
            }
        });

        // The listing is the only thing that varies between calls, so everything above it can be
        // cached: a large folder is many calls sharing one prefix.
        let system = "You are helping localize a video game. You will be given a listing of files \
                      from a game package: path, size, and what a mechanical check made of each. \
                      Say which ones are likely to hold text a player would read - dialogue, menus, \
                      item names - and which do not. You are given no file contents, so reason from \
                      names, sizes and formats, and say so when that is not enough. Prefer saying \
                      you are unsure over guessing: a wrong confident answer costs somebody an \
                      afternoon.";

        let listing = files
            .iter()
            .map(|f| {
                format!(
                    "{}\t{} bytes\t{}\t{}",
                    f.path,
                    f.size,
                    f.detected,
                    if f.readable {
                        "already readable"
                    } else {
                        "not read by this build"
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        self.call(
            api_key,
            serde_json::json!({
                "model": self.settings.model,
                "max_tokens": 16000,
                "thinking": { "type": "adaptive" },
                "system": [{
                    "type": "text",
                    "text": system,
                    "cache_control": { "type": "ephemeral" }
                }],
                "tools": [tool],
                "tool_choice": { "type": "tool", "name": "report_files" },
                "messages": [{
                    "role": "user",
                    "content": format!("path\tsize\tdetected\tstate\n{listing}")
                }]
            }),
        )
    }

    /// The request that asks what one file is.
    pub fn inspect_call(&self, api_key: &str, sample: &Sample) -> HttpCall {
        let tool = serde_json::json!({
            "name": "report_format",
            "description": "Report what this file is and where its text sits.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "format": { "type": "string" },
                    "whereTextIs": { "type": "string" },
                    "addressing": {
                        "type": "string",
                        "description": "How a translation would be addressed back to a place in the file."
                    },
                    "caveat": {
                        "type": "string",
                        "description": "What would make this answer wrong."
                    }
                },
                "required": ["format", "whereTextIs", "addressing", "caveat"],
                "additionalProperties": false
            }
        });

        let system = "You are helping localize a video game. You will be given the opening bytes of \
                      one file from a game package, as hex and as printable characters. Say what \
                      format it appears to be, where translatable text sits inside it, and how a \
                      translation would be addressed back to a place in the file. A confident wrong \
                      answer about a binary format is worse than no answer, so name what would make \
                      you wrong.";

        self.call(
            api_key,
            serde_json::json!({
                "model": self.settings.model,
                "max_tokens": 16000,
                "thinking": { "type": "adaptive" },
                "system": [{
                    "type": "text",
                    "text": system,
                    "cache_control": { "type": "ephemeral" }
                }],
                "tools": [tool],
                "tool_choice": { "type": "tool", "name": "report_format" },
                "messages": [{
                    "role": "user",
                    "content": format!(
                        "file: {}\nsize: {} bytes\nfirst {} bytes as hex:\n{}\n\nsame bytes as text:\n{}",
                        sample.path,
                        sample.size,
                        Sample::BYTES.min(sample.size as usize),
                        sample.head_hex,
                        sample.head_text
                    )
                }]
            }),
        )
    }

    /// The request that asks what is wrong with a set of approved translations.
    pub fn review_call(
        &self,
        api_key: &str,
        lines: &[ReviewLine],
        register: Option<&str>,
        glossary: &[(String, String)],
    ) -> HttpCall {
        let tool = serde_json::json!({
            "name": "report_problems",
            "description": "Report problems in these translations. Report nothing where nothing is wrong.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "notes": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "nodeId": { "type": "string" },
                                "kind": {
                                    "type": "string",
                                    "enum": ["register", "terminology", "context", "placeholder", "other"]
                                },
                                "detail": { "type": "string" },
                                "suggestion": { "type": "string" }
                            },
                            "required": ["nodeId", "kind", "detail"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["notes"],
                "additionalProperties": false
            }
        });

        let mut system = String::from(
            "You are reviewing the translation of a video game. For each line you are given the \
             original, the translation, and where in the game it appears. Report only what is \
             wrong: a register that does not suit the game, a term translated against the \
             project's own glossary, a line that reads as a generic translation rather than as \
             game text. Report nothing for a line that is fine - an empty list is the right \
             answer for good work. You are not rewriting anything; a person will decide.",
        );
        if let Some(register) = register {
            system.push_str("\n\nThe register this project has chosen: ");
            system.push_str(register);
        }
        if !glossary.is_empty() {
            system.push_str("\n\nTerms this project has settled on:\n");
            for (source, target) in glossary {
                system.push_str(&format!("  {source} = {target}\n"));
            }
        }

        let body = lines
            .iter()
            .map(|line| {
                format!(
                    "id: {}\ncontext: {}\nsource: {}\ntarget: {}\n",
                    line.node_id, line.context, line.source, line.target
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        self.call(
            api_key,
            serde_json::json!({
                "model": self.settings.model,
                "max_tokens": 16000,
                "thinking": { "type": "adaptive" },
                "system": [{
                    "type": "text",
                    "text": system,
                    "cache_control": { "type": "ephemeral" }
                }],
                "tools": [tool],
                "tool_choice": { "type": "tool", "name": "report_problems" },
                "messages": [{ "role": "user", "content": body }]
            }),
        )
    }

    /// The request that counts what a call would cost, without running it.
    ///
    /// A real count from the service rather than characters divided by four. Guessing a number and
    /// calling it an estimate is the kind of half-honesty this project does not do.
    pub fn count_call(&self, api_key: &str, call: &HttpCall) -> HttpCall {
        let mut body: serde_json::Value =
            serde_json::from_str(&call.body).unwrap_or(serde_json::Value::Null);
        // The counting endpoint takes the same request minus what only matters for generating.
        if let Some(object) = body.as_object_mut() {
            object.remove("max_tokens");
            object.remove("thinking");
        }
        self.call_to(
            &format!("{ENDPOINT}/count_tokens"),
            api_key,
            body.to_string(),
        )
    }
}

/// Reads the tool input out of a reply, or says why it could not.
///
/// Three ways a reply can fail to be an answer, and they are different things to a person: the
/// service reported an error, the model declined, or the shape was not what was asked for.
pub fn read_tool_input(reply: &str) -> Result<serde_json::Value, String> {
    let value: serde_json::Value =
        serde_json::from_str(reply).map_err(|e| format!("the reply was not JSON: {e}"))?;

    // A failed call often returns 200 with an error object rather than a failure status.
    if let Some(error) = value.get("error") {
        return Err(error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("the service reported an error")
            .to_string());
    }

    // Checked before the content is read: a refusal is a successful HTTP response whose content
    // is not the answer.
    if value.get("stop_reason").and_then(|s| s.as_str()) == Some("refusal") {
        let category = value
            .pointer("/stop_details/category")
            .and_then(|c| c.as_str())
            .unwrap_or("unspecified");
        return Err(format!("the model declined this request ({category})"));
    }

    value
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|blocks| {
            blocks
                .iter()
                .find(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
        })
        .and_then(|block| block.get("input").cloned())
        .ok_or_else(|| "the reply held no answer in the shape that was asked for".to_string())
}

/// Reads a survey reply.
pub fn read_survey(reply: &str, model: &str) -> Result<Survey, String> {
    let input = read_tool_input(reply)?;
    let verdicts = input
        .get("files")
        .cloned()
        .unwrap_or(serde_json::Value::Array(Vec::new()));
    let verdicts: Vec<FileVerdict> =
        serde_json::from_value(verdicts).map_err(|e| format!("the answer did not fit: {e}"))?;
    Ok(Survey {
        verdicts,
        model: model.to_string(),
    })
}

/// Reads an inspection reply.
pub fn read_inspection(reply: &str) -> Result<Inspection, String> {
    let input = read_tool_input(reply)?;
    serde_json::from_value(input).map_err(|e| format!("the answer did not fit: {e}"))
}

/// Reads a review reply.
pub fn read_review(reply: &str) -> Result<Vec<ReviewNote>, String> {
    let input = read_tool_input(reply)?;
    let notes = input
        .get("notes")
        .cloned()
        .unwrap_or(serde_json::Value::Array(Vec::new()));
    serde_json::from_value(notes).map_err(|e| format!("the answer did not fit: {e}"))
}

/// Reads a token count.
pub fn read_count(reply: &str) -> Result<u64, String> {
    let value: serde_json::Value =
        serde_json::from_str(reply).map_err(|e| format!("the reply was not JSON: {e}"))?;
    if let Some(error) = value.get("error") {
        return Err(error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("the service reported an error")
            .to_string());
    }
    value
        .get("input_tokens")
        .and_then(|t| t.as_u64())
        .ok_or_else(|| "the reply held no token count".to_string())
}
