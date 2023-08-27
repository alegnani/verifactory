mod model_entities;

use std::collections::HashMap;

use z3::{
    ast::{Ast, Int, Real},
    Config, Context, Optimize,
};

use crate::ir::FlowGraph;

use petgraph::prelude::{EdgeIndex, NodeIndex};

use self::model_entities::{Z3Edge, Z3Node};

pub struct Z3Backend {
    graph: FlowGraph,
    solver: Optimize<'static>,
}

pub struct BuildHelper {
    edge_map: HashMap<EdgeIndex, Real<'static>>,
    input_map: HashMap<NodeIndex, Int<'static>>,
    output_map: HashMap<NodeIndex, Real<'static>>,
    costs: Vec<Real<'static>>,
}

impl BuildHelper {
    pub fn new() -> Self {
        let edge_map = HashMap::new();
        let costs = Vec::new();
        let input_map = HashMap::new();
        let output_map = HashMap::new();
        Self {
            edge_map,
            costs,
            input_map,
            output_map,
        }
    }
}

impl Z3Backend {
    /* FIXME: this creates a memory leak */
    pub fn new(graph: FlowGraph) -> Self {
        let config = Config::new();
        let context = Box::new(Context::new(&config));
        /* non-halal stuff to keep the borrow-checker happy :/ */
        let context = Box::leak(context);
        let solver = Optimize::new(context);
        Self { graph, solver }
    }

    pub fn model(&self) -> (Real<'_>, BuildHelper) {
        let mut helper = BuildHelper::new();
        /* encode edges as variables in z3 */
        for edge_idx in self.graph.edge_indices() {
            let edge = &self.graph[edge_idx];
            edge.model(edge_idx, self, &mut helper);
        }

        /* encode edges as equations */
        for node_idx in self.graph.node_indices() {
            let node = &self.graph[node_idx];
            node.model(node_idx, self, &mut helper);
        }
        /* add up all the costs */
        let solver = &self.solver;
        let ctx = solver.get_context();
        let tot_cost = Real::new_const(ctx, "tot_cost");
        if !helper.costs.is_empty() {
            let cost_slice = helper.costs.iter().collect::<Vec<_>>();
            let cost_sum = Real::add(ctx, &cost_slice);
            solver.assert(&tot_cost._eq(&cost_sum));
            solver.minimize(&tot_cost);
        }
        (tot_cost, helper)
    }
}

#[cfg(test)]
mod test {
    use std::fs;

    use petgraph::dot::Dot;
    use z3::ast::Bool;

    use crate::{compiler::Compiler, import::string_to_entities, ir::Shrinkable};

    use super::*;

    fn load(file: &str) -> FlowGraph {
        let blueprint_string = fs::read_to_string(file).unwrap();
        let entities = string_to_entities(&blueprint_string).unwrap();
        Compiler::new(entities).create_graph()
    }

    #[test]
    fn test_balancer() {
        let graph = load("tests/broken-4-4");
        // let graph = graph.shrink(crate::ir::ShrinkStrength::Aggressive);
        println!("Graph:\n{:?}", Dot::with_config(&graph, &[]));
        let s = Z3Backend::new(graph);
        let (cost, h) = s.model();
        println!("Model:\n{}", s.solver);
        let outputs = h.output_map.values().collect::<Vec<_>>();

        let ctx = s.solver.get_context();

        let pairwise = outputs
            .windows(2)
            .map(|w| w[0]._eq(w[1]))
            .collect::<Vec<_>>();

        /* negate the output equality condition */
        let ref_slice = pairwise.iter().collect::<Vec<_>>();
        let output_eq = Bool::and(ctx, &ref_slice);
        let cost_cond = cost._eq(&Real::from_real(ctx, 0, 1));

        // let implc = cond.iff(&cost_cond);

        let res = s.solver.check(&[output_eq.not(), cost_cond]);
        // let res = s.solver.check(&[implc.clone()]);
        // println!("{:?}", res);
        // let res = s.solver.check(&[implc.not()]);
        println!("{:?}", res);
        let model = s.solver.get_model().unwrap();
        println!("{}", model);
    }

    #[test]
    fn priority_splitter() {
        let graph = load("tests/prio_splitter");
        // let graph = graph.shrink(crate::ir::ShrinkStrength::Aggressive);
        println!("Graph:\n{:?}", Dot::with_config(&graph, &[]));
        let s = Z3Backend::new(graph);
        let (cost, h) = s.model();
        println!("Model:\n{}", s.solver);
        let outputs = h.output_map.values().collect::<Vec<_>>();

        let ctx = s.solver.get_context();

        let pairwise = outputs
            .windows(2)
            .map(|w| w[0]._eq(w[1]))
            .collect::<Vec<_>>();

        /* negate the output equality condition */
        let ref_slice = pairwise.iter().collect::<Vec<_>>();
        let output_eq = Bool::and(ctx, &ref_slice);
        let cost_cond = cost._eq(&Real::from_real(ctx, 0, 1));

        // let implc = cond.iff(&cost_cond);

        let res = s.solver.check(&[output_eq.not(), cost_cond]);
        // let res = s.solver.check(&[implc.clone()]);
        // println!("{:?}", res);
        // let res = s.solver.check(&[implc.not()]);
        println!("{:?}", res);
        let model = s.solver.get_model().unwrap();
        println!("{}", model);
    }
}
