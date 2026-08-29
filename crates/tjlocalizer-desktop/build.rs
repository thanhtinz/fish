use std::path::Path;

fn main() {
    // Tauri embeds the built interface into the binary, so this crate cannot compile until the
    // frontend has been built. Tauri's own failure is a proc-macro panic deep in the output; this
    // says what to run instead, because a fresh clone hits it immediately and the built files are
    // not in version control.
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../desktop/dist");
    if !dist.exists() {
        println!("cargo:warning=the desktop interface has not been built yet");
        println!("cargo:warning=run: npm --prefix desktop install && npm --prefix desktop run build");
    }
    // Rebuild when the interface changes, so a stale bundle is never embedded.
    println!("cargo:rerun-if-changed=../../desktop/dist");

    tauri_build::build()
}
