//! Thanhtinz JAR Localizer - generic core.
//!
//! The design rule from the specification is that nothing in this crate may know about a
//! particular game: no game names, no class or package names, no resource paths, no screen
//! coordinates, no string IDs. The core understands *formats* - archives, class files, encodings,
//! text - and everything game-specific is expressed as detected capabilities, project rules or
//! plugins layered above it.

pub mod classfile;
pub mod error;

pub use error::{Error, Result};
