//! Lightweight editor module for in-place markdown editing.
//!
//! Provides a rope-backed text buffer with cursor management,
//! designed for integration into the TEA architecture.

mod buffer;

pub use buffer::{Cursor, Direction, EditorBuffer};
