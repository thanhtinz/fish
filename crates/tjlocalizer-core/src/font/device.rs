//! Making a game draw with the handset's own font instead of its glyph sheet (§16).
//!
//! Most J2ME games draw text from a strip of pixels holding the letters they were written for.
//! There are two ways to get Vietnamese into such a game, and until now this project only had the
//! first:
//!
//! 1. **Extend the sheet.** Compose the 134 letters from the game's own glyphs, install the sheet,
//!    and teach the game that it is taller. The letters look like the game's, which is the whole
//!    argument for it - and every step is per-game work, the last one especially.
//! 2. **Stop using the sheet.** Every MIDP handset can draw whatever its own font covers, and on
//!    anything sold after about 2005 that includes Vietnamese. A game switched to the device font
//!    needs no composition, no installation and no character order: it just draws.
//!
//! The second is what most people doing this by hand actually do, and it is usually the right
//! answer. What it costs is the look: the game's hand-drawn letters are replaced by the handset's,
//! which at twelve pixels is a visible change and sometimes an unacceptable one. What it buys is
//! everything else - and for a game whose sheet is CJK-only, where composing Vietnamese from
//! Chinese glyphs is not possible at all, it is the only answer.
//!
//! ## How the switch is made
//!
//! A game does not have a setting for this. It has a font class - `GFont`, `MyFont`, `F`, whatever
//! the author called it - with a method that blits characters out of an image, and everything else
//! calls that method. So the switch is made *there*: the body of that one method is replaced with a
//! direct call to `Graphics.drawString`, and every call site in the game keeps working unchanged
//! because nothing about the method's signature moved.
//!
//! This is the only place in this crate that writes bytecode, and it is fenced accordingly:
//!
//! - Only methods whose **shape** is recognised - a drawing surface, a string, and the numbers to
//!   place it at - are offered, because the body written has to match the arguments exactly.
//! - The written body has **no branches**, so it needs no stack map frames, so none have to be
//!   computed. `set_method_body` refuses a body that branches rather than trusting this.
//! - Nothing is switched on. Every candidate is offered with its evidence and becomes a rule that
//!   is off until a person turns it on (§19).
//!
//! ## Why the delegate is a parameter
//!
//! `Toolkit::midp()` names the MIDP methods, and that is what a real game gets. The indirection
//! exists so the machinery can be *proved*: no desktop JVM has `javax.microedition.lcdui`, so a
//! test that could only target MIDP could never be run against a verifier. Pointed at
//! `java.io.PrintStream` instead, the same rewrite produces a class an ordinary JVM loads,
//! verifies and runs - which is the only way to know the bytecode is right.

use crate::classfile::{ClassFile, MethodRef};
use crate::jar::Archive;
use crate::Result;
use serde::{Deserialize, Serialize};

/// A method the rewritten body will call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delegate {
    pub called: MethodRef,
    /// True for a method declared on an interface, which needs the other invoke instruction.
    pub interface: bool,
}

impl Delegate {
    fn new(owner: &str, name: &str, descriptor: &str) -> Self {
        Delegate {
            called: MethodRef {
                owner: owner.to_string(),
                name: name.to_string(),
                descriptor: descriptor.to_string(),
            },
            interface: false,
        }
    }
}

/// The platform methods a rewritten font class delegates to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toolkit {
    /// Drawing: called on the surface the game passed in.
    pub draw: Delegate,
    /// The static that hands back the device font.
    pub default_font: Delegate,
    /// Measuring, called on that font.
    pub string_width: Delegate,
    pub char_width: Delegate,
    pub height: Delegate,
}

impl Toolkit {
    /// What a MIDP handset offers.
    pub fn midp() -> Self {
        Toolkit {
            draw: Delegate::new(
                "javax/microedition/lcdui/Graphics",
                "drawString",
                "(Ljava/lang/String;III)V",
            ),
            default_font: Delegate::new(
                "javax/microedition/lcdui/Font",
                "getDefaultFont",
                "()Ljavax/microedition/lcdui/Font;",
            ),
            string_width: Delegate::new(
                "javax/microedition/lcdui/Font",
                "stringWidth",
                "(Ljava/lang/String;)I",
            ),
            char_width: Delegate::new("javax/microedition/lcdui/Font", "charWidth", "(C)I"),
            height: Delegate::new("javax/microedition/lcdui/Font", "getHeight", "()I"),
        }
    }
}

/// What one of a font class's methods does, as far as its shape says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Job {
    /// Draws a string on a surface it was handed.
    Draw,
    /// Answers how wide a string is.
    StringWidth,
    /// Answers how wide one character is.
    CharWidth,
    /// Answers how tall a line is.
    Height,
}

impl Job {
    pub fn key(self) -> &'static str {
        match self {
            Job::Draw => "draw",
            Job::StringWidth => "string-width",
            Job::CharWidth => "char-width",
            Job::Height => "height",
        }
    }
}

/// A method that could be switched to the device font.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// The archive entry holding the class.
    pub class: String,
    pub method: String,
    pub descriptor: String,
    pub job: Job,
    /// Why this class looks like a font: what it was seen calling. Evidence, for a person who can
    /// check it against a game they have run.
    pub evidence: Vec<String>,
}

/// How a game draws its text, as far as its constant pools say.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Strategy {
    /// It blits pieces of an image: a glyph sheet.
    pub bitmap: bool,
    /// It calls the platform's own text drawing.
    pub device: bool,
    pub evidence: Vec<String>,
}

impl Strategy {
    /// Whether switching to the device font is worth offering.
    ///
    /// A game already drawing with `drawString` and nothing else has nothing to switch: its
    /// Vietnamese works as soon as the strings are translated, and the font tab is a distraction.
    pub fn worth_switching(&self) -> bool {
        self.bitmap
    }
}

/// How this game draws text, from what its classes call.
pub fn strategy(archive: &Archive) -> Result<Strategy> {
    let mut bitmap = false;
    let mut device = false;
    let mut evidence: Vec<String> = Vec::new();

    for entry in archive.classes() {
        let Ok(class) = ClassFile::parse(&entry.data) else {
            continue;
        };
        for called in class.method_refs() {
            if !called.owner.ends_with("lcdui/Graphics") && !called.owner.ends_with("lcdui/Font") {
                continue;
            }
            let note = match called.name.as_str() {
                // Blitting a piece of an image is what drawing from a glyph sheet is.
                "drawRegion" | "drawSubstring" if called.owner.ends_with("Graphics") => {
                    bitmap = true;
                    format!("{} calls Graphics.{}", entry.name, called.name)
                }
                "drawString" | "drawChar" if called.owner.ends_with("Graphics") => {
                    device = true;
                    format!("{} calls Graphics.{}", entry.name, called.name)
                }
                "getDefaultFont" | "getFont" if called.owner.ends_with("Font") => {
                    device = true;
                    format!("{} asks for a device font", entry.name)
                }
                _ => continue,
            };
            if !evidence.contains(&note) {
                evidence.push(note);
            }
        }
        // Drawing an image inside a class that also holds one is weaker evidence than drawRegion
        // and worth having: a game that draws each glyph by clipping calls drawImage, not
        // drawRegion.
        if !bitmap
            && class
                .method_refs()
                .iter()
                .any(|c| c.owner.ends_with("lcdui/Graphics") && c.name == "setClip")
            && class
                .method_refs()
                .iter()
                .any(|c| c.owner.ends_with("lcdui/Graphics") && c.name == "drawImage")
        {
            bitmap = true;
            let note = format!("{} clips and draws an image, a glyph at a time", entry.name);
            if !evidence.contains(&note) {
                evidence.push(note);
            }
        }
    }
    evidence.sort();
    Ok(Strategy {
        bitmap,
        device,
        evidence,
    })
}

/// Every method in the game that could be switched to the device font.
///
/// Shape first, name second. A method taking a surface, a string and the numbers to place it at is
/// a drawing method whatever it is called; a method called `stringWidth` that takes a string and
/// returns an int is one too, and there the name is doing real work, because that shape alone is
/// far too common to act on.
pub fn candidates(archive: &Archive) -> Result<Vec<Candidate>> {
    let mut found = Vec::new();
    for entry in archive.classes() {
        let Ok(class) = ClassFile::parse(&entry.data) else {
            continue;
        };
        let refs = class.method_refs();
        let mut evidence: Vec<String> = Vec::new();
        for called in &refs {
            if called.owner.ends_with("lcdui/Graphics")
                && matches!(called.name.as_str(), "drawRegion" | "drawImage" | "setClip")
            {
                let note = format!("it calls Graphics.{}", called.name);
                if !evidence.contains(&note) {
                    evidence.push(note);
                }
            }
        }
        // A class that never touches a Graphics is not the class that draws the game's text.
        if evidence.is_empty() {
            continue;
        }
        evidence.sort();

        for method in class.methods()? {
            let Some(job) = job_of(&method.name, &method.descriptor) else {
                continue;
            };
            found.push(Candidate {
                class: entry.name.clone(),
                method: method.name.clone(),
                descriptor: method.descriptor.clone(),
                job,
                evidence: evidence.clone(),
            });
        }
    }
    found.sort_by(|a, b| {
        a.class
            .cmp(&b.class)
            .then(a.method.cmp(&b.method))
            .then(a.descriptor.cmp(&b.descriptor))
    });
    Ok(found)
}

/// What a method's shape and name say it does, if anything.
///
/// Public because a rule names a method rather than carrying its job: a rule written against one
/// version of a game and run against another must decide what it is looking at *now*, from the
/// class in front of it.
pub fn job_of(name: &str, descriptor: &str) -> Option<Job> {
    let lower = name.to_lowercase();
    let (parameters, returns) = split_descriptor(descriptor)?;

    // A surface, a string, and at least two numbers to place it at. That shape is specific enough
    // to act on without asking what the method is called.
    if returns == "V"
        && parameters.len() >= 4
        && parameters[0].starts_with('L')
        && parameters[1] == "Ljava/lang/String;"
        && parameters[2..].iter().take(2).all(|p| p == "I")
    {
        return Some(Job::Draw);
    }
    // The measuring methods. Their shapes are ordinary - anything can take a string and return an
    // int - so here the name has to agree.
    if returns == "I" && parameters == ["Ljava/lang/String;"] && lower.contains("width") {
        return Some(Job::StringWidth);
    }
    if returns == "I" && parameters == ["C"] && lower.contains("width") {
        return Some(Job::CharWidth);
    }
    if returns == "I" && parameters.is_empty() && lower.contains("height") {
        return Some(Job::Height);
    }
    None
}

/// Splits a method descriptor into its parameters and its return type.
fn split_descriptor(descriptor: &str) -> Option<(Vec<String>, String)> {
    let inner = descriptor.strip_prefix('(')?;
    let (parameters, returns) = inner.split_once(')')?;

    let mut out = Vec::new();
    let mut chars = parameters.chars().peekable();
    while let Some(c) = chars.next() {
        let mut type_name = String::new();
        let mut c = c;
        while c == '[' {
            type_name.push(c);
            c = chars.next()?;
        }
        type_name.push(c);
        if c == 'L' {
            for c in chars.by_ref() {
                type_name.push(c);
                if c == ';' {
                    break;
                }
            }
            if !type_name.ends_with(';') {
                return None;
            }
        }
        out.push(type_name);
    }
    Some((out, returns.to_string()))
}

/// How many local slots a type takes: two for a long or a double, one for everything else.
fn slots(type_name: &str) -> u16 {
    match type_name {
        "J" | "D" => 2,
        _ => 1,
    }
}

/// Rewrites one method to use the device font.
///
/// The class is changed in place and nothing else about it moves: the method keeps its name, its
/// descriptor and its place in the class, so every call site in the game keeps working.
pub fn rewrite(class: &mut ClassFile, candidate: &Candidate, toolkit: &Toolkit) -> Result<()> {
    let method = class
        .methods()?
        .into_iter()
        .find(|m| m.name == candidate.method && m.descriptor == candidate.descriptor)
        .ok_or_else(|| crate::Error::MalformedClassBody {
            reason: format!(
                "{} has no {}{}",
                candidate.class, candidate.method, candidate.descriptor
            ),
        })?;

    let (parameters, _) = split_descriptor(&candidate.descriptor).ok_or_else(|| {
        crate::Error::MalformedClassBody {
            reason: format!("{} is not a method descriptor", candidate.descriptor),
        }
    })?;

    // Where each parameter sits. A static method has no `this`, so everything shifts down one.
    let mut slot = if method.is_static { 0u16 } else { 1 };
    let mut at = Vec::new();
    for parameter in &parameters {
        at.push(slot);
        slot += slots(parameter);
    }
    let locals = slot.max(1);

    let (code, stack) = match candidate.job {
        Job::Draw => {
            let (delegate_parameters, _) = split_descriptor(&toolkit.draw.called.descriptor)
                .ok_or_else(|| crate::Error::MalformedClassBody {
                    reason: "the toolkit's drawing method has no descriptor".into(),
                })?;
            // How many numbers the platform's own drawing takes after the string. A MIDP
            // `drawString` takes three - x, y and the anchor - and a game's method often passes
            // only two, so the anchor is supplied.
            let wanted = delegate_parameters.len().saturating_sub(1);
            let available = parameters.len() - 2;

            let mut code = Vec::new();
            load_object(&mut code, at[0]);
            load_object(&mut code, at[1]);
            for i in 0..wanted {
                if i < available {
                    load_int(&mut code, at[2 + i]);
                } else if i + 1 == wanted {
                    // The anchor: top left, which is what a game blitting its own glyphs from a
                    // corner was already doing.
                    code.push(0x10); // bipush
                    code.push(ANCHOR_TOP_LEFT);
                } else {
                    return Err(crate::Error::MalformedClassBody {
                        reason: format!(
                            "{}{} does not carry the numbers {} needs",
                            candidate.method, candidate.descriptor, toolkit.draw.called.name
                        ),
                    });
                }
            }
            invoke(class, &mut code, &toolkit.draw);
            code.push(0xB1); // return
            let stack = 2 + wanted as u16;
            (code, stack)
        }

        Job::StringWidth | Job::CharWidth => {
            let mut code = Vec::new();
            invoke_static(class, &mut code, &toolkit.default_font);
            let call = if candidate.job == Job::StringWidth {
                load_object(&mut code, at[0]);
                &toolkit.string_width
            } else {
                load_int(&mut code, at[0]);
                &toolkit.char_width
            };
            invoke(class, &mut code, call);
            code.push(0xAC); // ireturn
            (code, 2)
        }

        Job::Height => {
            let mut code = Vec::new();
            invoke_static(class, &mut code, &toolkit.default_font);
            invoke(class, &mut code, &toolkit.height);
            code.push(0xAC); // ireturn
            (code, 1)
        }
    };

    class.set_method_body(
        &candidate.method,
        &candidate.descriptor,
        &code,
        stack,
        locals,
    )
}

/// `Graphics.TOP | Graphics.LEFT`, the anchor a game blitting from a corner was already using.
const ANCHOR_TOP_LEFT: u8 = 16 | 4;

fn load_object(code: &mut Vec<u8>, slot: u16) {
    match slot {
        0..=3 => code.push(0x2A + slot as u8), // aload_0 .. aload_3
        _ => {
            code.push(0x19); // aload
            code.push(slot as u8);
        }
    }
}

fn load_int(code: &mut Vec<u8>, slot: u16) {
    match slot {
        0..=3 => code.push(0x1A + slot as u8), // iload_0 .. iload_3
        _ => {
            code.push(0x15); // iload
            code.push(slot as u8);
        }
    }
}

fn invoke(class: &mut ClassFile, code: &mut Vec<u8>, delegate: &Delegate) {
    let index = class.add_method_ref(&delegate.called, delegate.interface);
    code.push(if delegate.interface { 0xB9 } else { 0xB6 });
    code.extend_from_slice(&index.to_be_bytes());
    if delegate.interface {
        // invokeinterface carries an argument count and a zero nobody reads any more.
        code.push(1);
        code.push(0);
    }
}

fn invoke_static(class: &mut ClassFile, code: &mut Vec<u8>, delegate: &Delegate) {
    let index = class.add_method_ref(&delegate.called, false);
    code.push(0xB8); // invokestatic
    code.extend_from_slice(&index.to_be_bytes());
}
