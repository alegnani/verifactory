mod backends;
mod compiler;
mod entities;
mod import;
mod ir;
mod utils;

use std::fs;

use backends::Z3Backend;
use ir::{ShrinkStrength, Shrinkable};
use petgraph::dot::Dot;

use crate::{compiler::Compiler, import::string_to_entities};

fn main() {
    /* FIXME: this does not work as there is no counterexample with cost == 0
     * given that some inputs of the mergers / outputs of the splitters are not present */
    let blueprint_string = fs::read_to_string("tests/3-2-broken").unwrap();
    let entities = string_to_entities(&blueprint_string).unwrap();
    let mut graph = Compiler::new(entities).create_graph();
    graph.coalesce_nodes(ShrinkStrength::Aggressive);
    println!("Graph:\n{:?}", Dot::with_config(&graph, &[]));
    let s = Z3Backend::new(graph);
    let is_not_balancer = s.is_not_belt_balancer(&[2, 4, 5, 6]);
    assert!(is_not_balancer);
}
