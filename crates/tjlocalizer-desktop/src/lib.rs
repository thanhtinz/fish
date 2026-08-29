//! The desktop application's own logic, as a library.
//!
//! A binary crate cannot be imported by an integration test, and the view models are worth
//! testing: they are where a display bug turns into a correctness one. So everything lives here
//! and `main.rs` is only the entry point.

pub mod commands;
pub mod csvfmt;
pub mod state;

/// Builds and runs the application.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::recent_projects,
            commands::import_jar,
            commands::open_project,
            commands::project_summary,
            commands::analyze,
            commands::capabilities,
            commands::extract,
            commands::nodes,
            commands::set_translation,
            commands::suggest_all,
            commands::apply_safe,
            commands::learn,
            commands::build,
            commands::builds,
            commands::rollback,
            commands::set_branding,
            commands::set_source_language,
            commands::set_style,
            commands::add_target,
            commands::remove_target,
            commands::build_all,
            commands::gloss,
            commands::export_build,
            commands::export_translations,
            commands::import_translations,
            commands::import_dictionary,
            commands::languages,
            commands::styles,
            commands::dictionaries,
        ])
        .run(tauri::generate_context!())
        .expect("the application failed to start");
}
