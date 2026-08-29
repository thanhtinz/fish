// The console window on Windows is for a terminal program; this is not one.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tjlocalizer_desktop_lib::run()
}
