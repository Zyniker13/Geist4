//! A stable, C-compatible API for the Geist4 Markdown engine: the base for
//! native bindings such as the Swift package.
//!
//! The boundary is designed so no panic can cross it: opaque handles,
//! explicit ownership, UTF-8 in and out, and entry points that catch and
//! return error codes. This crate is a shell until milestone M7; see the
//! repository roadmap.
