//! The three `bornes` (specs.md §3) — self-contained modules, each with its
//! own interception mechanism (or none at all, in `prosa`'s case, which is
//! just a pure function called by the other two).

pub mod comandos;
pub mod mcp;
pub mod prosa;
