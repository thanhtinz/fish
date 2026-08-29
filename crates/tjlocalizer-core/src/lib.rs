//! Thanhtinz JAR Localizer - generic core.
//!
//! The design rule from the specification is that nothing in this crate may know about a
//! particular game: no game names, no class or package names, no resource paths, no screen
//! coordinates, no string IDs. The core understands *formats* - archives, class files, encodings,
//! text - and everything game-specific is expressed as detected capabilities, project rules or
//! plugins layered above it.

pub mod assets;
pub mod build;
pub mod classfile;
pub mod claude;
pub mod context;
pub mod detect;
pub mod dictionary;
pub mod dictionary_data;
pub mod encoding;
pub mod error;
pub mod font;
pub mod graph;
pub mod jar;
pub mod lang;
pub mod locres;
pub mod package;
pub mod patch;
pub mod plugin;
pub mod project;
pub mod provider;
pub mod quality;
pub mod register;
pub mod regress;
pub mod resource;
pub mod rules;
pub mod secrets;
pub mod shorten;
pub mod suggest;
pub mod translate;
pub mod translation;
pub mod tree;
pub mod validate;
pub mod vietnamese;
pub mod writeback;

pub use error::{Error, Result};
