//! Proves a class whose *code* was patched still verifies and runs.
//!
//! `patch_demo` rewrites constants, which changes the text everywhere it is used. This changes
//! one use of one string and leaves the other alone - the case the constant pool cannot express -
//! by adding a constant and repointing a single load instruction. If the instruction's length or
//! the class's structure were got wrong, the JVM's verifier would reject the class rather than
//! print anything, which is exactly the proof wanted. Used by tools/verify-roundtrip.sh.

use tjlocalizer_core::classfile::ClassFile;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let input = args
        .next()
        .expect("usage: site_demo <in.class> <out.class>");
    let output = args
        .next()
        .expect("usage: site_demo <in.class> <out.class>");

    let mut class = ClassFile::parse(&std::fs::read(&input)?)?;

    // The fixture shows its quit label twice: once on the menu, once in the dialog that asks
    // whether you meant it. Vietnamese wants different words for the two.
    let sites: Vec<_> = class
        .string_sites()?
        .into_iter()
        .filter(|s| s.text.as_deref() == Some("Quit"))
        .collect();
    anyhow::ensure!(
        sites.len() == 2,
        "expected the fixture to load Quit twice, found {}",
        sites.len()
    );

    let confirm = class.add_string("Thoát khỏi trò chơi?")?;
    let on_the_menu = class.add_string("Thoát")?;
    for site in &sites {
        let index = if site.method == "main" {
            on_the_menu
        } else {
            confirm
        };
        class.point_site_at(site, index)?;
    }

    std::fs::write(&output, class.write()?)?;
    eprintln!("repointed {} load sites, changed no constant", sites.len());
    Ok(())
}
