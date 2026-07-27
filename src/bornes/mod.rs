//! Os três `bornes` (specs.md §3) — módulos autocontidos, cada um com seu
//! próprio mecanismo de interceptação (ou nenhum, no caso de `prosa`, que é
//! só uma função pura chamada pelos outros dois).

pub mod comandos;
pub mod mcp;
pub mod prosa;
