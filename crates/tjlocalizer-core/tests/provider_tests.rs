//! The network providers: what goes out, what comes back, and what is refused.
//!
//! No test here reaches the internet. The transport is pointed at a local server, or replaced
//! outright, so the request shaping and the reply handling are exercised without a service, a key
//! or a bill - and so the suite stays runnable offline.

use std::io::{Read, Write};
use std::net::TcpListener;
use tjlocalizer_core::lang::Language;
use tjlocalizer_core::provider::{Briefing, HttpProvider, ProviderConfig, ProviderKind};
use tjlocalizer_core::register;
use tjlocalizer_core::translate::{Completeness, Provider, Request};
use tjlocalizer_core::translation::{Glossary, GlossaryEntry};

fn request(text: &str, context: &str) -> Request {
    Request {
        source_text: text.into(),
        from: Language::new("en"),
        to: Language::new("vi-VN"),
        context: context.into(),
        placeholders: tjlocalizer_core::graph::find_placeholders(text),
        speaker: Default::default(),
        stance: Default::default(),
    }
}

fn config(kind: ProviderKind) -> ProviderConfig {
    ProviderConfig {
        enabled: true,
        kind,
        endpoint: kind.default_endpoint().to_string(),
        model: Some("test-model".into()),
        timeout_seconds: 5,
    }
}

fn glossary(entries: Vec<(&str, &str, bool)>) -> Glossary {
    Glossary {
        entries: entries
            .into_iter()
            .map(|(source, target, locked)| GlossaryEntry {
                source: source.into(),
                target: target.into(),
                locked,
                note: String::new(),
            })
            .collect(),
    }
}

/// A one-request HTTP server. Returns the port and everything it received.
///
/// Headers and body arrive in separate reads, so this keeps reading until it has as many bytes
/// as Content-Length promised - a single read returns the headers alone and makes any assertion
/// about the body fail for the wrong reason.
fn serve(reply: &'static str) -> (u16, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut received = String::new();
        let mut buffer = [0u8; 4096];
        loop {
            let read = stream.read(&mut buffer).unwrap();
            if read == 0 {
                break;
            }
            received.push_str(&String::from_utf8_lossy(&buffer[..read]));
            let Some((head, body)) = received.split_once("\r\n\r\n") else {
                continue;
            };
            let length: usize = head
                .lines()
                .find_map(|l| {
                    let (name, value) = l.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().ok())?
                })
                .unwrap_or(0);
            if body.len() >= length {
                break;
            }
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            reply.len(),
            reply
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.flush().unwrap();
        received
    });
    (port, handle)
}

/// Nothing may reach the network while the provider is switched off. This is the guarantee that
/// makes the offline default meaningful.
#[test]
fn a_disabled_provider_sends_nothing_and_proposes_nothing() {
    let mut config = config(ProviderKind::OpenAiCompatible);
    config.enabled = false;
    let glossary = Glossary::default();

    // A transport that fails the test if it is ever reached.
    let calls = std::cell::Cell::new(0usize);
    let provider = HttpProvider::new(
        config,
        "key".into(),
        Briefing {
            glossary: &glossary,
            style: None,
        },
    )
    .with_transport(|_| {
        calls.set(calls.get() + 1);
        Ok(String::new())
    });

    assert!(provider.propose(&request("Quit", "ui")).is_none());
    assert!(!provider.supports(&Language::new("en"), &Language::new("vi-VN")));
    assert_eq!(calls.get(), 0, "a disabled provider must not send anything");
}

/// The briefing is what turns a correct translation into a game one. Without the terminology and
/// the register, a good engine still says "thiết bị" and addresses a wuxia player as "bạn".
#[test]
fn the_request_carries_the_terminology_and_the_register() {
    let glossary = glossary(vec![("Guild", "bang hội", true)]);
    let style = register::builtin("natural-dialogue").unwrap();
    let provider = HttpProvider::new(
        config(ProviderKind::OpenAiCompatible),
        "key".into(),
        Briefing {
            glossary: &glossary,
            style: Some(&style),
        },
    );

    let instructions = provider.instructions(&request("Guild: %d members", "ui"));
    assert!(instructions.contains("Guild = bang hội"), "{instructions}");
    assert!(instructions.contains("ngươi"), "{instructions}");
    assert!(instructions.contains("%d"), "{instructions}");
    assert!(instructions.contains("Vietnamese"), "{instructions}");
    // The register's forbidden wording goes out too, not only checked coming back.
    assert!(
        instructions.contains("Never write \"bạn\""),
        "{instructions}"
    );
}

/// Interface text takes no pronoun, and an engine left to itself will insert one.
#[test]
fn interface_text_is_told_to_use_no_pronouns() {
    let glossary = Glossary::default();
    let style = register::builtin("terse-ui").unwrap();
    let provider = HttpProvider::new(
        config(ProviderKind::OpenAiCompatible),
        "key".into(),
        Briefing {
            glossary: &glossary,
            style: Some(&style),
        },
    );
    let instructions = provider.instructions(&request("Settings", "ui"));
    assert!(
        instructions.contains("no personal pronouns"),
        "{instructions}"
    );
}

/// A round trip over a real socket: the request is formed, sent, and the reply parsed.
#[test]
fn an_openai_shaped_reply_is_read() {
    let (port, server) = serve(r#"{"choices":[{"message":{"content":"Bắt đầu trò chơi"}}]}"#);
    let mut config = config(ProviderKind::OpenAiCompatible);
    config.endpoint = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let glossary = Glossary::default();

    let provider = HttpProvider::new(
        config,
        "secret-key".into(),
        Briefing {
            glossary: &glossary,
            style: None,
        },
    );
    let proposal = provider.propose(&request("Start Game", "ui")).unwrap();
    assert_eq!(proposal.target_text, "Bắt đầu trò chơi");
    assert_eq!(proposal.completeness, Completeness::Complete);

    let sent = server.join().unwrap();
    // Header names are case-insensitive and ureq sends them lowercased.
    assert!(
        sent.to_lowercase()
            .contains("authorization: bearer secret-key"),
        "{sent}"
    );
    assert!(sent.contains("Start Game"), "{sent}");
}

#[test]
fn each_family_is_read_from_its_own_shape() {
    for (kind, reply, expected) in [
        (
            ProviderKind::DeepL,
            r#"{"translations":[{"text":"Thoát"}]}"#,
            "Thoát",
        ),
        (
            ProviderKind::GoogleV2,
            r#"{"data":{"translations":[{"translatedText":"Thoát"}]}}"#,
            "Thoát",
        ),
        (
            ProviderKind::LibreTranslate,
            r#"{"translatedText":"Thoát"}"#,
            "Thoát",
        ),
    ] {
        let (port, _server) = serve(reply);
        let mut config = config(kind);
        config.endpoint = format!("http://127.0.0.1:{port}/");
        let glossary = Glossary::default();
        let provider = HttpProvider::new(
            config,
            "key".into(),
            Briefing {
                glossary: &glossary,
                style: None,
            },
        );
        let proposal = provider.propose(&request("Quit", "ui")).unwrap();
        assert_eq!(proposal.target_text, expected, "{kind:?}");
    }
}

/// The one refusal that is not negotiable. A translation that dropped a format argument would
/// crash the game, and no amount of fluency makes it usable.
#[test]
fn a_reply_that_lost_a_placeholder_is_refused_rather_than_offered() {
    let glossary = Glossary::default();
    let provider = HttpProvider::new(
        config(ProviderKind::OpenAiCompatible),
        "key".into(),
        Briefing {
            glossary: &glossary,
            style: None,
        },
    )
    .with_transport(|_| Ok(r#"{"choices":[{"message":{"content":"Sinh lực"}}]}"#.to_string()));

    let proposal = provider.propose(&request("HP: %d / %d", "format")).unwrap();
    assert_eq!(proposal.completeness, Completeness::None);
    assert!(proposal.target_text.is_empty());
    assert!(
        proposal.notes[0].contains("%d"),
        "the reason must name what was lost: {:?}",
        proposal.notes
    );
}

/// Engines do not reliably follow a briefing, so the same terms are checked coming back.
#[test]
fn a_reply_that_ignored_a_settled_term_is_flagged_but_still_offered() {
    let glossary = glossary(vec![("Guild", "bang hội", true)]);
    let provider = HttpProvider::new(
        config(ProviderKind::OpenAiCompatible),
        "key".into(),
        Briefing {
            glossary: &glossary,
            style: None,
        },
    )
    .with_transport(|_| Ok(r#"{"choices":[{"message":{"content":"Hiệp hội"}}]}"#.to_string()));

    let proposal = provider.propose(&request("Guild", "ui")).unwrap();
    assert_eq!(proposal.target_text, "Hiệp hội");
    assert!(
        proposal.notes.iter().any(|n| n.contains("bang hội")),
        "{:?}",
        proposal.notes
    );
    // Flagged, not rewritten: the sentence may have been built around the wrong reading.
    assert!(proposal.confidence < 0.7);
}

/// A modern pronoun in a wuxia game is the most common way a machine translation gives itself
/// away, and it survives every grammatical check.
#[test]
fn a_reply_that_breaks_the_register_is_flagged() {
    let glossary = Glossary::default();
    let style = register::builtin("natural-dialogue").unwrap();
    let provider = HttpProvider::new(
        config(ProviderKind::OpenAiCompatible),
        "key".into(),
        Briefing {
            glossary: &glossary,
            style: Some(&style),
        },
    )
    .with_transport(|_| {
        Ok(r#"{"choices":[{"message":{"content":"Bạn có chắc không?"}}]}"#.to_string())
    });

    let proposal = provider
        .propose(&request("Are you sure?", "dialogue"))
        .unwrap();
    assert!(
        proposal.notes.iter().any(|n| n.contains("register")),
        "{:?}",
        proposal.notes
    );
    assert_eq!(proposal.register.as_deref(), Some("natural-dialogue"));
}

/// Engines add quotes despite being told not to, and a quoted string patched into a game shows
/// the quotes.
#[test]
fn quotes_the_engine_added_are_stripped() {
    for reply in [
        r#"{"choices":[{"message":{"content":"\"Thoát\""}}]}"#,
        "{\"choices\":[{\"message\":{\"content\":\"\u{201c}Thoát\u{201d}\"}}]}",
        r#"{"choices":[{"message":{"content":"  Thoát\n"}}]}"#,
    ] {
        let glossary = Glossary::default();
        let provider = HttpProvider::new(
            config(ProviderKind::OpenAiCompatible),
            "key".into(),
            Briefing {
                glossary: &glossary,
                style: None,
            },
        )
        .with_transport(move |_| Ok(reply.to_string()));
        assert_eq!(
            provider
                .propose(&request("Quit", "ui"))
                .unwrap()
                .target_text,
            "Thoát"
        );
    }
}

/// A service that answers 200 with an error object must not read as "no translation".
#[test]
fn an_error_object_is_reported_as_the_service_worded_it() {
    let glossary = Glossary::default();
    let provider = HttpProvider::new(
        config(ProviderKind::OpenAiCompatible),
        "key".into(),
        Briefing {
            glossary: &glossary,
            style: None,
        },
    )
    .with_transport(|_| Ok(r#"{"error":{"message":"Incorrect API key provided"}}"#.to_string()));

    let proposal = provider.propose(&request("Quit", "ui")).unwrap();
    assert_eq!(proposal.completeness, Completeness::None);
    assert!(
        proposal.notes[0].contains("Incorrect API key"),
        "{:?}",
        proposal.notes
    );
}

#[test]
fn an_unreachable_service_says_so_rather_than_failing_silently() {
    let mut config = config(ProviderKind::OpenAiCompatible);
    // Port 1 is not listening, and binding it would need privileges, so nothing can answer.
    config.endpoint = "http://127.0.0.1:1/".into();
    config.timeout_seconds = 2;
    let glossary = Glossary::default();

    let provider = HttpProvider::new(
        config,
        "key".into(),
        Briefing {
            glossary: &glossary,
            style: None,
        },
    );
    let proposal = provider.propose(&request("Quit", "ui")).unwrap();
    assert_eq!(proposal.completeness, Completeness::None);
    assert!(
        proposal.notes[0].contains("could not be reached"),
        "{:?}",
        proposal.notes
    );
}

/// Even a clean reply is a proposal. Fluent and wrong is the failure mode of a machine
/// translation, and nothing in the reply tells it from fluent and right.
#[test]
fn no_machine_translation_is_ever_auto_approvable() {
    let glossary = Glossary::default();
    let provider = HttpProvider::new(
        config(ProviderKind::OpenAiCompatible),
        "key".into(),
        Briefing {
            glossary: &glossary,
            style: None,
        },
    )
    .with_transport(|_| Ok(r#"{"choices":[{"message":{"content":"Thoát"}}]}"#.to_string()));

    let proposal = provider.propose(&request("Quit", "ui")).unwrap();
    assert!(!proposal.is_approvable());
    assert!(proposal.confidence <= 0.7);
}

#[test]
fn deepl_gets_its_own_tag_spelling() {
    let glossary = Glossary::default();
    let provider = HttpProvider::new(
        config(ProviderKind::DeepL),
        "key".into(),
        Briefing {
            glossary: &glossary,
            style: None,
        },
    );
    let mut request = request("Quit", "ui");
    request.to = Language::new("en-GB");
    assert!(provider.build_call(&request).body.contains("EN-GB"));

    request.to = Language::new("pt-BR");
    assert!(provider.build_call(&request).body.contains("PT-BR"));
}

/// Google takes its key in the query string; the others take a header. Getting this wrong is an
/// authentication failure that looks like an outage.
#[test]
fn each_family_carries_its_key_where_that_family_expects_it() {
    let glossary = Glossary::default();
    let briefing = || Briefing {
        glossary: &glossary,
        style: None,
    };

    let google = HttpProvider::new(config(ProviderKind::GoogleV2), "K".into(), briefing());
    let call = google.build_call(&request("Quit", "ui"));
    assert!(call.url.ends_with("?key=K"), "{}", call.url);
    assert!(!call.headers.iter().any(|(n, _)| n == "Authorization"));

    let deepl = HttpProvider::new(config(ProviderKind::DeepL), "K".into(), briefing());
    let call = deepl.build_call(&request("Quit", "ui"));
    assert!(call
        .headers
        .iter()
        .any(|(n, v)| n == "Authorization" && v == "DeepL-Auth-Key K"));
    assert!(!call.url.contains('K'), "the key must not be in the URL");
}

/// The name in the file and the name the interface uses must be the same string, or a settings
/// file spells a provider differently from the tool that wrote it.
#[test]
fn the_stored_name_matches_the_name_shown() {
    for kind in ProviderKind::all() {
        let written = serde_json::to_string(&kind).unwrap();
        assert_eq!(
            written.trim_matches('"'),
            kind.id(),
            "{kind:?} serialises differently from its id"
        );
        let read: ProviderKind = serde_json::from_str(&written).unwrap();
        assert_eq!(read, kind);
    }
}

/// A settings file written by an earlier build must still open.
///
/// This is not hypothetical: renaming these variants made an existing project unreadable, and the
/// project then vanished from the recent list without a word.
#[test]
fn names_an_earlier_build_wrote_are_still_readable() {
    for (written, expected) in [
        ("open-ai-compatible", ProviderKind::OpenAiCompatible),
        ("deep-l", ProviderKind::DeepL),
        ("libre-translate", ProviderKind::LibreTranslate),
        ("google-v2", ProviderKind::GoogleV2),
        ("anthropic", ProviderKind::Anthropic),
    ] {
        let read: ProviderKind = serde_json::from_str(&format!("\"{written}\""))
            .unwrap_or_else(|e| panic!("a project written with {written:?} no longer opens: {e}"));
        assert_eq!(read, expected);
    }
}

/// Anthropic takes its key in `x-api-key`, and the instructions go in the system block.
///
/// The system block rather than the first message on purpose: the instructions are the same for
/// every string in a run, so putting them where they can be cached is the difference between one
/// bill and several.
#[test]
fn anthropic_carries_its_key_in_a_header_and_its_briefing_in_the_system_block() {
    let glossary = glossary(vec![("Quit", "Thoát", false)]);
    let provider = HttpProvider::new(
        config(ProviderKind::Anthropic),
        "K".into(),
        Briefing {
            glossary: &glossary,
            style: None,
        },
    );
    let call = provider.build_call(&request("Quit", "ui"));

    assert_eq!(call.url, "https://api.anthropic.com/v1/messages");
    assert!(call
        .headers
        .iter()
        .any(|(n, v)| n == "x-api-key" && v == "K"));
    assert!(call
        .headers
        .iter()
        .any(|(n, v)| n == "anthropic-version" && v == "2023-06-01"));
    assert!(!call.url.contains('K'), "the key must not be in the URL");

    let body: serde_json::Value = serde_json::from_str(&call.body).unwrap();
    let system = body.pointer("/system/0/text").unwrap().as_str().unwrap();
    assert!(system.contains("Thoát"), "the glossary should reach it");
    assert_eq!(
        body.pointer("/system/0/cache_control/type").unwrap(),
        "ephemeral"
    );
    assert_eq!(body.pointer("/messages/0/content").unwrap(), "Quit");
}

/// A family with a model named must send the name; one without must fall back to the family's own
/// default rather than to an empty string, which reads as a service outage.
#[test]
fn a_family_with_no_model_named_uses_its_own_default() {
    let glossary = Glossary::default();
    let mut settings = config(ProviderKind::Anthropic);
    settings.model = None;
    let provider = HttpProvider::new(
        settings,
        "K".into(),
        Briefing {
            glossary: &glossary,
            style: None,
        },
    );
    let body: serde_json::Value =
        serde_json::from_str(&provider.build_call(&request("Quit", "ui")).body).unwrap();
    assert_eq!(body.get("model").unwrap(), "claude-opus-5");
}

/// A decline arrives as a successful response, so it is checked before the content is read.
#[test]
fn anthropic_reports_a_decline_rather_than_no_translation() {
    let glossary = Glossary::default();
    let provider = HttpProvider::new(
        config(ProviderKind::Anthropic),
        "K".into(),
        Briefing {
            glossary: &glossary,
            style: None,
        },
    )
    .with_transport(|_| Ok(r#"{"stop_reason":"refusal","content":[]}"#.to_string()));

    // A refusal comes back as a proposal with no text and the reason attached, the same shape
    // every other failure takes here - so it reaches a person rather than being read as silence.
    let proposal = provider.propose(&request("Quit", "ui")).unwrap();
    assert!(proposal.target_text.is_empty());
    assert!(!proposal.is_approvable());
    assert!(
        proposal.notes.iter().any(|n| n.contains("declined")),
        "{:?}",
        proposal.notes
    );
}

/// What comes back from Anthropic is still a proposal, and a proposal is never approvable.
#[test]
fn an_anthropic_translation_is_still_only_a_proposal() {
    let glossary = Glossary::default();
    let provider = HttpProvider::new(
        config(ProviderKind::Anthropic),
        "K".into(),
        Briefing {
            glossary: &glossary,
            style: None,
        },
    )
    .with_transport(|_| {
        Ok(r#"{"stop_reason":"end_turn","content":[{"type":"text","text":"Thoát"}]}"#.to_string())
    });

    let proposal = provider.propose(&request("Quit", "ui")).unwrap();
    assert_eq!(proposal.target_text, "Thoát");
    assert!(!proposal.is_approvable());
}
