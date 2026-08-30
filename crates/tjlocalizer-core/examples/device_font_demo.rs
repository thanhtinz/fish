//! Proves a class whose font method was rewritten still verifies and runs.
//!
//! Most J2ME games draw their text from a glyph sheet, and the second way of getting Vietnamese
//! into one is to stop doing that: replace the body of the game's own drawing method with a call
//! to the handset's, and every call site in the game keeps working because nothing about the
//! method's signature moved.
//!
//! That is the only place in this crate that writes bytecode, so it is the place that most needs
//! a verifier's opinion rather than a test's. No desktop JVM has `javax.microedition.lcdui`, so
//! the toolkit is pointed at `java.io.PrintStream` and `java.lang.String` instead: the same
//! rewrite, the same decisions about local slots and stack depth, against a class an ordinary JVM
//! will load. Used by tools/verify-roundtrip.sh.

use tjlocalizer_core::classfile::{ClassFile, MethodRef};
use tjlocalizer_core::font::device::{self, Candidate, Delegate, Job, Toolkit};

fn delegate(owner: &str, name: &str, descriptor: &str) -> Delegate {
    Delegate {
        called: MethodRef {
            owner: owner.to_string(),
            name: name.to_string(),
            descriptor: descriptor.to_string(),
        },
        interface: false,
    }
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let input = args
        .next()
        .expect("usage: device_font_demo <in.class> <out.class>");
    let output = args
        .next()
        .expect("usage: device_font_demo <in.class> <out.class>");

    // A desktop stand-in for what a handset offers: something to draw with, a static that hands
    // back a thing, and methods on that thing which measure.
    let toolkit = Toolkit {
        draw: delegate("java/io/PrintStream", "println", "(Ljava/lang/String;)V"),
        default_font: delegate("java/lang/System", "lineSeparator", "()Ljava/lang/String;"),
        string_width: delegate("java/lang/String", "compareTo", "(Ljava/lang/String;)I"),
        char_width: delegate("java/lang/String", "indexOf", "(I)I"),
        height: delegate("java/lang/String", "length", "()I"),
    };

    let mut class = ClassFile::parse(&std::fs::read(&input)?)?;
    let jobs = [
        (
            "drawString",
            "(Ljava/io/PrintStream;Ljava/lang/String;II)V",
            Job::Draw,
        ),
        ("stringWidth", "(Ljava/lang/String;)I", Job::StringWidth),
        ("getHeight", "()I", Job::Height),
    ];
    for (method, descriptor, job) in jobs {
        device::rewrite(
            &mut class,
            &Candidate {
                class: input.clone(),
                method: method.to_string(),
                descriptor: descriptor.to_string(),
                job,
                evidence: Vec::new(),
            },
            &toolkit,
        )?;
    }

    std::fs::write(&output, class.write()?)?;
    eprintln!(
        "rewrote {} methods to use the platform's own font",
        jobs.len()
    );
    Ok(())
}
