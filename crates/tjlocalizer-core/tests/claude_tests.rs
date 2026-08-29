//! Asking a model, and the three rules that hold while doing it.
//!
//! No test here reaches the internet. The transport is replaced by a closure, exactly as the
//! translation engine's is, so what a request contains and what a reply does are exercised without
//! a service, a key or a bill.
//!
//! Two of these tests assert *negatives* - that nothing was sent, and that a file's contents are
//! not in the request. A self-made fixture proves a negative completely, because every byte in it
//! is one this test put there.

use std::cell::Cell;
use tjlocalizer_core::claude::{self, Analyst, FileFact, ReviewLine, Sample, Settings};
use tjlocalizer_core::jar::Archive;

/// The distinctive bytes are the point: every assertion below looks for them by name.
const DIALOGUE: &str = "start=Cast your line into the water\nquit=Pack up and go home\n";
/// A real one-pixel PNG, carrying its marker in a tEXt chunk. Real rather than a stub because a
/// twenty-byte fake is not binary enough to be refused, and a fixture that flatters the check
/// proves nothing: `secretpixels` below is inside genuine image bytes.
#[rustfmt::skip]
const OPAQUE: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xde, 0x00, 0x00, 0x00, 0x14, 0x74, 0x45, 0x58, 0x74, 0x43, 0x6f, 0x6d, 0x6d, 0x65, 0x6e, 0x74,
    0x00, 0x73, 0x65, 0x63, 0x72, 0x65, 0x74, 0x70, 0x69, 0x78, 0x65, 0x6c, 0x73, 0xa2, 0x4f, 0x95,
    0x03, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0x00,
    0x00, 0x03, 0x01, 0x01, 0x00, 0xc9, 0xfe, 0x92, 0xef, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
    0x44, 0xae, 0x42, 0x60, 0x82,
];

fn package() -> Archive {
    let mut archive = Archive::read(&empty_zip()).unwrap();
    archive.insert("assets/strings.properties", DIALOGUE.as_bytes().to_vec());
    archive.insert("assets/title.png", OPAQUE.to_vec());
    archive.insert("classes.dex", b"dex\n035\0some bytecode".to_vec());
    archive
}

fn empty_zip() -> Vec<u8> {
    // The end-of-central-directory record of a zip with nothing in it.
    let mut bytes = b"PK\x05\x06".to_vec();
    bytes.extend_from_slice(&[0u8; 18]);
    bytes
}

fn on() -> Settings {
    Settings {
        enabled: true,
        model: "claude-opus-5".into(),
        timeout_seconds: 5,
    }
}

fn a_survey_reply() -> String {
    serde_json::json!({
        "stop_reason": "tool_use",
        "content": [{
            "type": "tool_use",
            "name": "report_files",
            "input": {
                "files": [{
                    "path": "assets/title.png",
                    "holdsText": true,
                    "why": "a title image usually carries the game's name as artwork",
                    "confidence": 0.7
                }]
            }
        }]
    })
    .to_string()
}

/// The one that matters most: off is not a preference, it is a guarantee.
///
/// The same assertion the translation engine has, made the same way - by counting how many times
/// the transport was reached, so a future refactor that sends first and checks afterwards fails
/// here rather than in somebody's network log.
#[test]
fn nothing_is_sent_while_the_engine_is_off() {
    let calls = Cell::new(0);
    let analyst = Analyst::new(Settings::default()).with_transport(|_| {
        calls.set(calls.get() + 1);
        Ok(a_survey_reply())
    });

    let facts = claude::facts(&package());
    let survey = analyst.survey("key", &facts);
    let inspect = analyst.inspect("key", &Sample::of("a.bin", b"\0\0"));
    let review = analyst.review("key", &[], None, &[]);

    assert_eq!(
        calls.get(),
        0,
        "the transport was reached with the engine off"
    );
    for outcome in [survey.err(), inspect.err(), review.err()] {
        assert_eq!(outcome.as_deref(), Some(claude::OFF));
    }
}

/// The privacy boundary, asserted against the bytes themselves.
///
/// A scan sends names, sizes and what the mechanical check made of each file. It does not send
/// what is inside them, and the check for that is to look for the contents in the request body.
#[test]
fn a_scan_sends_names_and_never_contents() {
    let sent = std::cell::RefCell::new(String::new());
    let analyst = Analyst::new(on()).with_transport(|call| {
        *sent.borrow_mut() = call.body.clone();
        Ok(a_survey_reply())
    });

    let facts = claude::facts(&package());
    analyst.survey("key", &facts).unwrap();
    let body = sent.borrow();

    assert!(
        body.contains("assets/strings.properties"),
        "the name should go"
    );
    assert!(body.contains("assets/title.png"));

    // The text file's own words, and the binary's, are what must not be there.
    assert!(
        !body.contains("Cast your line into the water"),
        "a file's contents reached the request"
    );
    assert!(!body.contains("Pack up and go home"));
    assert!(!body.contains("secretpixels"));
}

/// Only the file asked about, and only the beginning of it.
#[test]
fn an_inspection_sends_one_file_and_a_bounded_piece_of_it() {
    let long = vec![b'A'; Sample::BYTES * 4];
    let sample = Sample::of("assets/unknown.bin", &long);

    assert_eq!(sample.size, (Sample::BYTES * 4) as u64);
    assert_eq!(sample.head_hex.len(), Sample::BYTES * 2);
    assert_eq!(sample.head_text.len(), Sample::BYTES);

    let sent = std::cell::RefCell::new(String::new());
    let analyst = Analyst::new(on()).with_transport(|call| {
        *sent.borrow_mut() = call.body.clone();
        Err("stop here".to_string())
    });
    let _ = analyst.inspect("key", &sample);

    let body = sent.borrow();
    assert!(body.contains("assets/unknown.bin"));
    // Four times the sample would be there if the bound were not applied.
    assert!(
        !body.contains(&"41".repeat(Sample::BYTES + 1)),
        "more than the sample was sent"
    );
}

/// A suggestion never becomes a finding.
///
/// The model is told, in as many words, that a PNG holds text. The mechanical decision about that
/// file must be exactly what it was before, because writing a file back on a guess destroys it.
#[test]
fn a_suggestion_does_not_change_what_the_package_says() {
    let archive = package();
    let before = tjlocalizer_core::package::detect(&archive);

    let analyst = Analyst::new(on()).with_transport(|_| Ok(a_survey_reply()));
    let survey = analyst.survey("key", &claude::facts(&archive)).unwrap();
    assert!(survey.verdicts[0].holds_text, "the model said yes");

    let after = tjlocalizer_core::package::detect(&archive);
    assert_eq!(
        before
            .readable
            .iter()
            .map(|r| r.entry.clone())
            .collect::<Vec<_>>(),
        after
            .readable
            .iter()
            .map(|r| r.entry.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        !after.readable.iter().any(|r| r.entry.ends_with(".png")),
        "a guess was promoted into the readable list"
    );

    // And the decision that governs writing is untouched.
    let png = archive.get("assets/title.png").unwrap();
    assert!(matches!(
        tjlocalizer_core::writeback::plan(&png.name, &png.data),
        tjlocalizer_core::writeback::Plan::ReadOnly { .. }
    ));
}

/// A decline is a successful HTTP response whose content is not the answer.
#[test]
fn a_refusal_is_reported_as_one() {
    let reply = serde_json::json!({
        "stop_reason": "refusal",
        "stop_details": { "category": "policy" },
        "content": []
    })
    .to_string();

    let analyst = Analyst::new(on()).with_transport(move |_| Ok(reply.clone()));
    let outcome = analyst.survey("key", &[]);
    let message = outcome.unwrap_err();
    assert!(message.contains("declined"), "{message}");
    assert!(message.contains("policy"), "{message}");
}

#[test]
fn an_error_object_is_reported_as_the_service_worded_it() {
    let reply = serde_json::json!({
        "type": "error",
        "error": { "type": "invalid_request_error", "message": "credit balance is too low" }
    })
    .to_string();

    let analyst = Analyst::new(on()).with_transport(move |_| Ok(reply.clone()));
    assert_eq!(
        analyst.survey("key", &[]).unwrap_err(),
        "credit balance is too low"
    );
}

/// A reply in the wrong shape gets a sentence, not a panic and not a silent empty result.
#[test]
fn a_reply_in_the_wrong_shape_is_reported_readably() {
    let analyst = Analyst::new(on()).with_transport(|_| Ok("{\"content\":[]}".into()));
    let message = analyst.survey("key", &[]).unwrap_err();
    assert!(message.contains("shape"), "{message}");

    let analyst = Analyst::new(on()).with_transport(|_| Ok("not json at all".into()));
    let message = analyst.survey("key", &[]).unwrap_err();
    assert!(message.contains("not JSON"), "{message}");
}

/// A batch that fails does not throw away the batches that answered.
#[test]
fn one_bad_batch_does_not_lose_the_good_ones() {
    let seen = Cell::new(0);
    let analyst = Analyst::new(on()).with_transport(|_| {
        seen.set(seen.get() + 1);
        if seen.get() == 2 {
            Err("the service timed out".into())
        } else {
            Ok(a_survey_reply())
        }
    });

    let facts: Vec<FileFact> = (0..claude::BATCH * 3)
        .map(|i| FileFact {
            path: format!("assets/file-{i}.bin"),
            size: 10,
            detected: "unknown".into(),
            readable: false,
        })
        .collect();

    let (survey, trouble) = analyst.survey_all("key", &facts);
    assert_eq!(seen.get(), 3);
    assert_eq!(trouble.len(), 1);
    assert_eq!(survey.verdicts.len(), 2, "the two good batches were kept");
}

/// The review is the one path that sends the game's own text, so it says what it carries.
#[test]
fn a_review_carries_the_lines_and_the_terms_that_occur_in_them() {
    let sent = std::cell::RefCell::new(String::new());
    let analyst = Analyst::new(on()).with_transport(|call| {
        *sent.borrow_mut() = call.body.clone();
        Ok(serde_json::json!({
            "stop_reason": "tool_use",
            "content": [{
                "type": "tool_use",
                "name": "report_problems",
                "input": { "notes": [{
                    "nodeId": "n1",
                    "kind": "register",
                    "detail": "too formal for a fishing game",
                    "suggestion": "Quăng cần đi nào"
                }] }
            }]
        })
        .to_string())
    });

    let lines = vec![ReviewLine {
        node_id: "n1".into(),
        context: "ui".into(),
        source: "Cast your line".into(),
        target: "Thả dây câu".into(),
    }];
    let notes = analyst
        .review(
            "key",
            &lines,
            Some("plain, warm, second person"),
            &[("line".into(), "dây câu".into())],
        )
        .unwrap();

    let body = sent.borrow();
    assert!(body.contains("Cast your line"));
    assert!(body.contains("plain, warm, second person"));
    assert!(body.contains("dây câu"));

    // Notes, not edits. Nothing here writes to a translation store.
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].kind, "register");
    assert_eq!(notes[0].suggestion, "Quăng cần đi nào");
}

/// The counting request is the generating one minus what only matters for generating.
#[test]
fn a_count_asks_the_service_rather_than_dividing_by_four() {
    let analyst = Analyst::new(on());
    let call = analyst.survey_call("key", &claude::facts(&package()));
    let counting = analyst.count_call("key", &call);

    assert!(counting.url.ends_with("/count_tokens"));
    let body: serde_json::Value = serde_json::from_str(&counting.body).unwrap();
    assert!(body.get("max_tokens").is_none());
    assert!(body.get("thinking").is_none());
    assert!(
        body.get("tools").is_some(),
        "the tools are part of the count"
    );

    let analyst = Analyst::new(on()).with_transport(|_| Ok("{\"input_tokens\": 1234}".into()));
    assert_eq!(analyst.count("key", &call).unwrap(), 1234);
}

/// What a scan would send, listed before it is sent - and it excludes what extraction excludes.
#[test]
fn the_facts_are_the_whole_of_what_would_go() {
    let facts = claude::facts(&package());
    let names: Vec<&str> = facts.iter().map(|f| f.path.as_str()).collect();

    assert!(names.contains(&"assets/strings.properties"));
    assert!(names.contains(&"assets/title.png"));
    assert!(
        !names.iter().any(|n| n.starts_with("META-INF/")),
        "a package's own metadata is not game text on any platform"
    );

    let strings = facts
        .iter()
        .find(|f| f.path == "assets/strings.properties")
        .unwrap();
    assert!(strings.readable, "this build reads properties files");
    assert_eq!(strings.size, DIALOGUE.len() as u64);

    let dex = facts.iter().find(|f| f.path == "classes.dex").unwrap();
    assert!(!dex.readable);
    // The reason travels with it, so the model is told what is already known.
    assert!(!dex.detected.is_empty());
}
