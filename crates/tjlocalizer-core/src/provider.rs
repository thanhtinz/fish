//! Translation engines reached over the network (specification §32).
//!
//! This is the half of the answer a dictionary cannot give: an engine that translates sentences.
//! Three things shape the design, and none of them is negotiable.
//!
//! **The user's text leaves their machine.** So this is off unless switched on, never runs by
//! itself, and the interface says what goes where. The key lives outside the project directory,
//! because project.json is a file people commit and send to each other.
//!
//! **No engine is built in.** Which service, at what price, under whose terms, is the user's
//! decision. What is built in is the shape of the request for the common API families, so
//! configuring one is a URL and a key rather than a plugin.
//!
//! **A machine translation of a game is not a game translation.** An engine that knows nothing
//! about this game will render `装备` as "thiết bị", `Guild` as "hiệp hội", and address a wuxia
//! player as "bạn". So the project's glossary and register go *into* the request, and every reply
//! is checked against them coming back. That checking, not the engine, is what makes the output
//! usable - and it applies to whichever engine the user picked.

use crate::lang::Language;
use crate::register::StyleProfile;
use crate::translate::{Completeness, Proposal, Provider, Request, ResolvedTerm};
use crate::translation::{Glossary, Issue};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// The API families this build knows how to talk to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    /// Anything speaking OpenAI's `/chat/completions`: OpenAI itself, and the many services and
    /// local runtimes that copied it. The only family that can be told about register and
    /// terminology in words, which is why it is the default.
    ///
    /// Named explicitly because kebab-case would derive `open-ai-compatible`, which disagrees
    /// with `id()` - and a file that spells a setting differently from the interface that wrote
    /// it is a bug report waiting to happen. The derived spellings stay readable as aliases: a
    /// settings file written by an earlier build must still open, and finding out otherwise means
    /// a user's project has become unreadable.
    #[serde(rename = "openai-compatible", alias = "open-ai-compatible")]
    OpenAiCompatible,
    /// DeepL's `/v2/translate`.
    #[serde(rename = "deepl", alias = "deep-l")]
    DeepL,
    /// Google Cloud Translation v2.
    GoogleV2,
    /// LibreTranslate, including a self-hosted one - the option where nothing leaves a network
    /// the user controls.
    #[serde(rename = "libretranslate", alias = "libre-translate")]
    LibreTranslate,
}

impl ProviderKind {
    pub fn id(self) -> &'static str {
        match self {
            ProviderKind::OpenAiCompatible => "openai-compatible",
            ProviderKind::DeepL => "deepl",
            ProviderKind::GoogleV2 => "google-v2",
            ProviderKind::LibreTranslate => "libretranslate",
        }
    }

    /// Whether the family can be given instructions in prose. Only these can be told the register
    /// to write in; for the others the glossary and register are checked on the way back only.
    pub fn takes_instructions(self) -> bool {
        matches!(self, ProviderKind::OpenAiCompatible)
    }

    /// A sensible endpoint to start from. Editable: self-hosted deployments are the point.
    pub fn default_endpoint(self) -> &'static str {
        match self {
            ProviderKind::OpenAiCompatible => "https://api.openai.com/v1/chat/completions",
            ProviderKind::DeepL => "https://api-free.deepl.com/v2/translate",
            ProviderKind::GoogleV2 => "https://translation.googleapis.com/language/translate/v2",
            ProviderKind::LibreTranslate => "http://localhost:5000/translate",
        }
    }

    pub fn all() -> Vec<ProviderKind> {
        vec![
            ProviderKind::OpenAiCompatible,
            ProviderKind::DeepL,
            ProviderKind::GoogleV2,
            ProviderKind::LibreTranslate,
        ]
    }
}

/// How to reach an engine.
///
/// Deliberately without the key. This is written into the application's own configuration, which
/// a user may still copy about; the key is stored separately so that copying a configuration
/// cannot leak it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    /// Off unless the user turned it on. Nothing reaches the network while this is false.
    #[serde(default)]
    pub enabled: bool,
    pub kind: ProviderKind,
    pub endpoint: String,
    /// Model name, for the families that take one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Seconds to wait before giving up.
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    30
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: ProviderKind::OpenAiCompatible,
            endpoint: ProviderKind::OpenAiCompatible
                .default_endpoint()
                .to_string(),
            model: Some("gpt-4o-mini".to_string()),
            timeout_seconds: default_timeout(),
        }
    }
}

/// What the engine is told about this game, so its answer is a game translation rather than a
/// correct one.
pub struct Briefing<'a> {
    pub glossary: &'a Glossary,
    pub style: Option<&'a StyleProfile>,
}

/// An engine reached over HTTP.
pub struct HttpProvider<'a> {
    pub config: ProviderConfig,
    pub api_key: String,
    pub briefing: Briefing<'a>,
    /// Swapped out in tests for a local server. Nothing else may set this.
    transport: Box<dyn Fn(HttpCall) -> Result<String, String> + 'a>,
}

/// One outgoing request, as data, so the shaping of it can be tested without a network.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpCall {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub timeout: Duration,
}

impl<'a> HttpProvider<'a> {
    pub fn new(config: ProviderConfig, api_key: String, briefing: Briefing<'a>) -> Self {
        Self {
            config,
            api_key,
            briefing,
            transport: Box::new(send),
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

    /// The request this configuration would send for a string.
    ///
    /// Public so the interface can show it: a user about to send their game's text to a third
    /// party should be able to see exactly what would go.
    pub fn build_call(&self, request: &Request) -> HttpCall {
        let timeout = Duration::from_secs(self.config.timeout_seconds);
        let (headers, body) = match self.config.kind {
            ProviderKind::OpenAiCompatible => (
                vec![
                    ("Content-Type".into(), "application/json".into()),
                    ("Authorization".into(), format!("Bearer {}", self.api_key)),
                ],
                serde_json::json!({
                    "model": self.config.model.clone().unwrap_or_else(|| "gpt-4o-mini".into()),
                    // Zero temperature: the same line must not translate two ways in one game.
                    "temperature": 0.0,
                    "messages": [
                        { "role": "system", "content": self.instructions(request) },
                        { "role": "user", "content": request.source_text.clone() }
                    ]
                })
                .to_string(),
            ),
            ProviderKind::DeepL => (
                vec![
                    ("Content-Type".into(), "application/json".into()),
                    (
                        "Authorization".into(),
                        format!("DeepL-Auth-Key {}", self.api_key),
                    ),
                ],
                serde_json::json!({
                    "text": [request.source_text.clone()],
                    "target_lang": deepl_tag(&request.to),
                    "source_lang": deepl_tag(&request.from),
                    // Game strings are fragments, not prose; splitting them invents sentences.
                    "split_sentences": "0",
                    "preserve_formatting": true,
                })
                .to_string(),
            ),
            ProviderKind::GoogleV2 => (
                vec![("Content-Type".into(), "application/json".into())],
                serde_json::json!({
                    "q": request.source_text.clone(),
                    "source": request.from.base(),
                    "target": request.to.base(),
                    "format": "text",
                })
                .to_string(),
            ),
            ProviderKind::LibreTranslate => (
                vec![("Content-Type".into(), "application/json".into())],
                serde_json::json!({
                    "q": request.source_text.clone(),
                    "source": request.from.base(),
                    "target": request.to.base(),
                    "format": "text",
                    "api_key": self.api_key,
                })
                .to_string(),
            ),
        };

        let url = match self.config.kind {
            // Google v2 takes its key in the query string rather than a header.
            ProviderKind::GoogleV2 => format!("{}?key={}", self.config.endpoint, self.api_key),
            _ => self.config.endpoint.clone(),
        };

        HttpCall {
            url,
            headers,
            body,
            timeout,
        }
    }

    /// What an instruction-following engine is told.
    ///
    /// This is where a generic translation becomes a game one. Without the terminology and the
    /// register, a good engine still returns "thiết bị" for 装备 and addresses a wuxia player as
    /// "bạn", and the result reads as a machine's however fluent the grammar.
    pub fn instructions(&self, request: &Request) -> String {
        let mut lines = vec![
            format!(
                "You are translating text from a video game, from {} into {}.",
                request.from.display_name(),
                request.to.display_name()
            ),
            "Reply with the translation only: no quotes, no explanation, no alternatives."
                .to_string(),
        ];

        if !request.placeholders.is_empty() {
            lines.push(format!(
                "Keep these placeholders exactly as they are, in the same number: {}. \
                 They are format arguments and the game will break without them.",
                request.placeholders.join(", ")
            ));
        }

        lines.push(format!(
            "This string appears in the game's {} text. Keep it about as short as the original; \
             it has to fit where the original fitted.",
            request.context
        ));

        if let Some(style) = self.briefing.style {
            lines.push(format!("Register: {}", style.description));
            let pronouns = style.pronouns(request.speaker, request.stance);
            if !pronouns.second_singular.is_empty() {
                lines.push(format!(
                    "Address the player as \"{}\" and refer to the speaker as \"{}\". \
                     Do not use any other pronoun for them.",
                    pronouns.second_singular, pronouns.first_singular
                ));
            } else {
                lines.push(
                    "This is interface text with no speaker: use no personal pronouns at all."
                        .to_string(),
                );
            }
            for (avoid, instead) in &style.avoid {
                lines.push(format!("Never write \"{avoid}\"; write \"{instead}\"."));
            }
        }

        let terms = self.briefing.glossary.matches_in(&request.source_text);
        if !terms.is_empty() {
            lines.push("Use these renderings exactly; they are settled for this game:".to_string());
            for term in terms {
                lines.push(format!("  {} = {}", term.source, term.target));
            }
        }

        lines.join("\n")
    }

    /// Pulls the translation out of whatever the family returns.
    fn read_reply(&self, body: &str) -> Result<String, String> {
        let value: serde_json::Value =
            serde_json::from_str(body).map_err(|e| format!("the reply was not JSON: {e}"))?;

        // A failed call often returns 200 with an error object; saying so beats "no translation".
        if let Some(error) = value.get("error") {
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("the service reported an error");
            return Err(message.to_string());
        }

        let text = match self.config.kind {
            ProviderKind::OpenAiCompatible => value
                .pointer("/choices/0/message/content")
                .and_then(|v| v.as_str()),
            ProviderKind::DeepL => value
                .pointer("/translations/0/text")
                .and_then(|v| v.as_str()),
            ProviderKind::GoogleV2 => value
                .pointer("/data/translations/0/translatedText")
                .and_then(|v| v.as_str()),
            ProviderKind::LibreTranslate => value.get("translatedText").and_then(|v| v.as_str()),
        };

        text.map(tidy)
            .filter(|t| !t.is_empty())
            .ok_or_else(|| "the reply held no translation".to_string())
    }
}

impl Provider for HttpProvider<'_> {
    fn name(&self) -> &str {
        self.config.kind.id()
    }

    fn supports(&self, _from: &Language, _to: &Language) -> bool {
        // Whether the service actually covers a pair is the service's business, and it will say
        // so in its reply. Claiming otherwise here would silently hide a language it does handle.
        self.config.enabled
    }

    fn propose(&self, request: &Request) -> Option<Proposal> {
        if !self.config.enabled {
            return None;
        }

        let call = self.build_call(request);
        let body = match (self.transport)(call) {
            Ok(body) => body,
            Err(e) => {
                return Some(failed(&format!(
                    "{} could not be reached: {e}",
                    self.name()
                )))
            }
        };
        let text = match self.read_reply(&body) {
            Ok(text) => text,
            Err(e) => return Some(failed(&format!("{}: {e}", self.name()))),
        };

        Some(self.check(request, text))
    }
}

impl HttpProvider<'_> {
    /// Everything the engine's reply is held to before it is shown as a proposal.
    ///
    /// The engine is not trusted to have followed the briefing - none of them reliably do - so the
    /// same terminology and register that went out are checked coming back, and a lost placeholder
    /// is fatal rather than a note: the alternative is a proposal that would crash the game.
    fn check(&self, request: &Request, text: String) -> Proposal {
        let mut notes = Vec::new();
        let mut terms = Vec::new();

        let lost: Vec<&String> = request
            .placeholders
            .iter()
            .filter(|p| {
                text.matches(p.as_str()).count() != request.source_text.matches(p.as_str()).count()
            })
            .collect();

        if !lost.is_empty() {
            return failed(&format!(
                "{} returned a translation that lost {} - refused, because applying it would \
                 break the game at runtime",
                self.name(),
                lost.iter()
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        for entry in self.briefing.glossary.matches_in(&request.source_text) {
            if entry.locked && !text.contains(&entry.target) {
                notes.push(format!(
                    "the settled term {:?} should read {:?} and does not appear",
                    entry.source, entry.target
                ));
            } else if text.contains(&entry.target) {
                terms.push(ResolvedTerm {
                    source: entry.source.clone(),
                    target: entry.target.clone(),
                    domain: crate::dictionary::Domain::General,
                    fit: 1.0,
                });
            }
        }

        if let Some(style) = self.briefing.style {
            for Issue { detail, .. } in style.check(&text) {
                notes.push(detail);
            }
        }
        for Issue { code, detail } in crate::quality::check(
            &request.source_text,
            &text,
            &request.placeholders,
            &request.from,
            &request.to,
        ) {
            if code != "placeholder" {
                notes.push(detail);
            }
        }

        // Fluent and wrong is the failure mode of a machine translation, and nothing in the reply
        // distinguishes it from fluent and right. So the confidence stays middling however clean
        // the checks come back, and `is_approvable` is false regardless.
        let confidence = if notes.is_empty() { 0.7 } else { 0.4 };

        Proposal {
            target_text: text,
            completeness: Completeness::Complete,
            confidence,
            engine: self.name().to_string(),
            terms,
            unresolved: Vec::new(),
            register: self.briefing.style.map(|s| s.id.clone()),
            notes,
        }
    }
}

/// A proposal that carries only a reason. Returned rather than `None` so the interface can say
/// what went wrong instead of showing an empty panel.
fn failed(reason: &str) -> Proposal {
    Proposal {
        target_text: String::new(),
        completeness: Completeness::None,
        confidence: 0.0,
        engine: "provider".into(),
        terms: Vec::new(),
        unresolved: Vec::new(),
        register: None,
        notes: vec![reason.to_string()],
    }
}

/// Strips what an instruction-following engine adds despite being asked not to.
fn tidy(text: &str) -> String {
    let trimmed = text.trim();
    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .or_else(|| {
            trimmed
                .strip_prefix('\u{201c}')
                .and_then(|t| t.strip_suffix('\u{201d}'))
        })
        .unwrap_or(trimmed);
    unquoted.trim().to_string()
}

/// DeepL wants its own tag spelling: `EN-GB`, `PT-BR`, `ZH`.
fn deepl_tag(language: &Language) -> String {
    match (language.base().as_str(), language.region().as_deref()) {
        ("en", Some(region)) => format!("EN-{region}"),
        ("pt", Some(region)) => format!("PT-{region}"),
        (base, _) => base.to_uppercase(),
    }
}

/// The real transport.
fn send(call: HttpCall) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(call.timeout))
        .build()
        .into();

    let mut request = agent.post(&call.url);
    for (name, value) in &call.headers {
        request = request.header(name, value);
    }
    let mut response = request.send(&call.body).map_err(|e| e.to_string())?;
    response
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())
}
