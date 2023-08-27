mod backends;
mod compiler;
mod entities;
mod import;
mod ir;
mod utils;

use std::fs;

use backends::Z3Backend;
use ir::{ShrinkStrength, Shrinkable};

use crate::{compiler::Compiler, import::string_to_entities};

fn main() {
    let blueprint = fs::read_to_string("tests/belts").unwrap();
    let entities = string_to_entities(&blueprint).unwrap();
    let ctx = Compiler::new(entities);
    let graph = ctx.create_graph().shrink(ShrinkStrength::Aggressive);
    let mut solver = Z3Backend::new(graph);
    solver.model();
}
