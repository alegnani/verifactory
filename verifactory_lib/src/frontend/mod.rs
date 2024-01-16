//! Front-end used to convert a Factorio blueprint to the IR

mod compile_entities;
mod compile_graph;

pub use compile_graph::{Compiler, RelMap};
