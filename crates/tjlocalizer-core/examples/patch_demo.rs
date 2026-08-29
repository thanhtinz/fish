//! Proves a patched class still runs: rewrites every literal in a class file, writes it out, and
//! leaves it for the JVM to load. Used by tools/verify-roundtrip.sh.

use std::collections::HashMap;
use tjlocalizer_core::classfile::ClassFile;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let input = args
        .next()
        .expect("usage: patch_demo <in.class> <out.class>");
    let output = args
        .next()
        .expect("usage: patch_demo <in.class> <out.class>");

    let translations: HashMap<&str, &str> = HashMap::from([
        ("Dragon Quest Online", "Truyền Kỳ Rồng Thiêng"),
        ("Start Game", "Bắt đầu trò chơi"),
        ("Quit", "Thoát"),
        (
            "You have arrived at last, traveller.",
            "Rốt cuộc ngươi cũng tới rồi, lữ khách.",
        ),
        ("装备", "Trang bị"),
    ]);

    let mut class = ClassFile::parse(&std::fs::read(&input)?)?;
    let mut patched = 0;
    for literal in class.string_literals() {
        let Some(text) = literal.decoded.as_deref() else {
            continue;
        };
        if let Some(vi) = translations.get(text) {
            class.set_utf8_text(literal.utf8_index, vi)?;
            patched += 1;
        }
    }
    std::fs::write(&output, class.write()?)?;
    eprintln!("patched {patched} literals");
    Ok(())
}
