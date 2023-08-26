mod base_entity;
mod compiler;
mod entities;
mod import;
mod ir;
mod utils;

use std::fs;

use crate::{compiler::Compiler, import::string_to_entities};

fn main() {
    let blueprint = fs::read_to_string("tests/belts").unwrap();
    let entities = string_to_entities(&blueprint).unwrap();
    let ctx = Compiler::new(entities);
    let _inputs = ctx.generate_ir_inputs();
    let _outputs = ctx.generate_ir_outputs();
}
