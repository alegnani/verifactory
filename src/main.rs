mod base_entity;
mod entities;
mod import;
mod ir;
mod utils;

use std::fs;

use crate::import::string_to_entities;

fn main() {
    let blueprint = fs::read_to_string("test_blueprint").unwrap();
    let entities = string_to_entities(&blueprint).unwrap();
    println!("{:?}", entities);
}
