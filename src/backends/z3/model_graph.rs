use petgraph::prelude::{EdgeIndex, NodeIndex};
use std::collections::HashMap;
use z3::{
    ast::{Ast, Int, Real},
    Config, Context, SatResult, Solver,
};

use crate::ir::FlowGraph;

use super::model_entities::{Z3Edge, Z3Node};

pub struct Z3Helper<'a> {
    pub edge_map: HashMap<EdgeIndex, Real<'a>>,
    pub input_map: HashMap<NodeIndex, Int<'a>>,
    pub output_map: HashMap<NodeIndex, Real<'a>>,
}

impl<'a> Z3Helper<'a> {
    pub fn new() -> Self {
        let edge_map = HashMap::new();
        let input_map = HashMap::new();
        let output_map = HashMap::new();
        Self {
            edge_map,
            input_map,
            output_map,
        }
    }
}

pub struct Z3Backend {
    graph: FlowGraph,
    solver: Solver<'static>,
    // idx_to_id: HashMap<NodeIndex, EntityId>,
}

impl Z3Backend {
    /* FIXME: this creates a memory leak */
    pub fn new(graph: FlowGraph) -> Self {
        let config = Config::new();
        let context = Box::new(Context::new(&config));
        /* non-halal stuff to keep the borrow-checker happy :/ */
        let context = Box::leak(context);
        let solver = Solver::new(context);
        // let idx_to_id = graph
        //     .node_indices()
        //     .map(|idx| (idx, graph[idx].get_id()))
        //     .collect::<HashMap<_, _>>();

        Self { graph, solver }
    }

    pub fn get_solver(&self) -> &Solver {
        &self.solver
    }

    pub fn get_ctx(&self) -> &Context {
        self.solver.get_context()
    }

    pub fn get_graph(&self) -> &FlowGraph {
        &self.graph
    }

    pub fn model(&self) -> Z3Helper {
        let mut helper = Z3Helper::new();
        /* encode edges as variables in z3 */
        for edge_idx in self.graph.edge_indices() {
            let edge = &self.graph[edge_idx];
            let edge_const = edge.model(edge_idx, self);
            helper.edge_map.insert(edge_idx, edge_const);
        }
        /* encode nodes as equations */
        for node_idx in self.graph.node_indices() {
            let node = &self.graph[node_idx];
            helper = node.model(node_idx, self, helper);
        }
        println!("Solver:\n{}", self.solver);
        helper
    }
}
