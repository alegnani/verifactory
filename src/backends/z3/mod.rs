mod model_entities;

use std::collections::HashMap;

use z3::{
    ast::{Ast, Bool, Int, Real},
    Config, Context, Optimize, SatResult,
};

use crate::{entities::EntityId, ir::FlowGraph};

use petgraph::prelude::{EdgeIndex, NodeIndex};

use self::model_entities::{Z3Edge, Z3Node};

pub struct Z3Backend {
    graph: FlowGraph,
    solver: Optimize<'static>,
    idx_to_id: HashMap<NodeIndex, EntityId>,
}

pub struct BuildHelper {
    edge_map: HashMap<EdgeIndex, Real<'static>>,
    input_map: HashMap<NodeIndex, Int<'static>>,
    output_map: HashMap<NodeIndex, Real<'static>>,
    splitter_costs: Vec<Real<'static>>,
    merger_costs: Vec<Real<'static>>,
}

impl BuildHelper {
    pub fn new() -> Self {
        let edge_map = HashMap::new();
        let splitter_costs = Vec::new();
        let merger_costs = Vec::new();
        let input_map = HashMap::new();
        let output_map = HashMap::new();
        Self {
            edge_map,
            splitter_costs,
            merger_costs,
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
        let idx_to_id = graph
            .node_indices()
            .map(|idx| (idx, graph[idx].get_id()))
            .collect::<HashMap<_, _>>();

        Self {
            graph,
            solver,
            idx_to_id,
        }
    }

    pub fn model(&self) -> (Real<'_>, Real<'_>, BuildHelper) {
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
        /* merger costs */
        let tot_merger_cost = Real::new_const(ctx, "merger_tot_cost");
        if !helper.merger_costs.is_empty() {
            let cost_slice = helper.merger_costs.iter().collect::<Vec<_>>();
            let cost_sum = Real::add(ctx, &cost_slice);
            solver.assert(&tot_merger_cost._eq(&cost_sum));
            solver.minimize(&tot_merger_cost);
        }
        /* splitter costs */
        let tot_splitter_cost = Real::new_const(ctx, "splitter_tot_cost");
        if !helper.splitter_costs.is_empty() {
            let cost_slice = helper.splitter_costs.iter().collect::<Vec<_>>();
            let cost_sum = Real::add(ctx, &cost_slice);
            solver.assert(&tot_splitter_cost._eq(&cost_sum));
            solver.minimize(&tot_splitter_cost);
        }
        (tot_merger_cost, tot_splitter_cost, helper)
    }
}
impl Z3Backend {
    pub fn is_not_belt_balancer(&self, exclude_list: &[EntityId]) -> bool {
        let solver = &self.solver;
        let ctx = solver.get_context();

        let (_, cost, h) = self.model();

        /* remove exluded inputs and outputs */
        let (outputs, excluded_outputs) = self.remove_excluded(&h.output_map, exclude_list);
        let (_, excluded_inputs) = self.remove_excluded(&h.input_map, exclude_list);

        let zero = Int::from_i64(ctx, 0);
        for excluded in excluded_inputs {
            solver.assert(&excluded._eq(&zero));
        }
        for excluded in excluded_outputs {
            solver.assert(&excluded._eq(&Real::from_int(&zero)));
        }

        /* create the belt balancer predicate: all outputs are equal */
        let pairwise_out_eq = outputs
            .windows(2)
            .map(|w| w[0]._eq(w[1]))
            .collect::<Vec<_>>();
        let out_eq = pairwise_out_eq.iter().collect::<Vec<_>>();

        /* negate the output equality condition */
        let output_diseq = Bool::and(ctx, out_eq.as_slice()).not();
        let cost_cond = cost._eq(&Real::from_real(ctx, 0, 1));

        println!("Model:\n{}", solver);

        let res = solver.check(&[output_diseq, cost_cond]);
        println!("{:?}", res);
        if let SatResult::Sat = res {
            println!("Model:\n{}", solver.get_model().unwrap());
            return true;
        }
        false
    }

    fn remove_excluded<'a, T>(
        &self,
        map: &'a HashMap<NodeIndex, T>,
        exclude_list: &[EntityId],
    ) -> (Vec<&'a T>, Vec<&'a T>)
    where
        T: Ast<'a>,
    {
        let mut list = Vec::new();
        let mut excluded = Vec::new();
        for (k, v) in map {
            if exclude_list.contains(self.idx_to_id.get(k).unwrap()) {
                excluded.push(v);
            } else {
                list.push(v);
            }
        }
        (list, excluded)
    }

    pub fn is_not_equal_drain_belt_balancer(&self, exclude_list: &[EntityId]) -> bool {
        let solver = &self.solver;
        let ctx = solver.get_context();

        let (cm, _, h) = self.model();

        /* remove exluded inputs and outputs */
        let (outputs, excluded_outputs) = self.remove_excluded(&h.output_map, exclude_list);
        let (inputs, excluded_inputs) = self.remove_excluded(&h.input_map, exclude_list);

        let zero = Int::from_i64(ctx, 0);
        for excluded in excluded_inputs {
            solver.assert(&excluded._eq(&zero));
        }
        for excluded in excluded_outputs {
            solver.assert(&excluded._eq(&Real::from_int(&zero)));
        }

        /* create the belt balancer predicate: all outputs are equal */
        let pairwise_out_eq = outputs
            .windows(2)
            .map(|w| w[0]._eq(w[1]))
            .collect::<Vec<_>>();
        let out_eq = pairwise_out_eq.iter().collect::<Vec<_>>();

        /* create the belt balancer equal drain predicate: all inputs are equal */
        let pairwise_in_eq = inputs
            .windows(2)
            .map(|w| w[0]._eq(w[1]))
            .collect::<Vec<_>>();
        let in_eq = pairwise_in_eq.iter().collect::<Vec<_>>();

        /* negate the input equality condition */
        let input_diseq = Bool::and(ctx, in_eq.as_slice()).not();
        let output_eq = Bool::and(ctx, out_eq.as_slice());

        let cost = Real::add(ctx, &[&cm]);
        let cost_cond = cost._eq(&Real::from_real(ctx, 0, 1));

        println!("Model:\n{}", solver);

        let res = solver.check(&[output_eq, input_diseq, cost_cond]);
        if let SatResult::Sat = res {
            println!("Model:\n{}", solver.get_model().unwrap());
            return true;
        }
        false
    }
}

#[cfg(test)]
mod test {
    use std::fs;

    use petgraph::dot::Dot;
    use z3::{ast::Bool, SatResult};

    use crate::{compiler::Compiler, import::string_to_entities, ir::Shrinkable};

    use super::*;

    fn load(file: &str) -> FlowGraph {
        let blueprint_string = fs::read_to_string(file).unwrap();
        let entities = string_to_entities(&blueprint_string).unwrap();
        Compiler::new(entities).create_graph()
    }

    #[test]
    fn test_balancer() {
        let graph = load("tests/3-2");
        let graph = graph.shrink(crate::ir::ShrinkStrength::Aggressive);
        println!("Graph:\n{:?}", Dot::with_config(&graph, &[]));
        let s = Z3Backend::new(graph);
        let is_not_balancer = s.is_not_belt_balancer(&[]);
        assert!(is_not_balancer);
    }

    #[test]
    fn is_not_balancer() {
        let graph = load("tests/3-3-broken");
        let graph = graph.shrink(crate::ir::ShrinkStrength::Aggressive);
        println!("Graph:\n{:?}", Dot::with_config(&graph, &[]));
        let s = Z3Backend::new(graph);
        let is_not_balancer = s.is_not_belt_balancer(&[]);
        assert!(is_not_balancer);
    }

    #[test]
    fn is_balancer() {
        let graph = load("tests/3-3");
        let graph = graph.shrink(crate::ir::ShrinkStrength::Aggressive);
        println!("Graph:\n{:?}", Dot::with_config(&graph, &[]));
        let s = Z3Backend::new(graph);
        let is_not_balancer = s.is_not_belt_balancer(&[]);
        assert!(!is_not_balancer);
    }

    #[test]
    fn is_not_equal_drain_belt_balancer() {
        let graph = load("tests/3-2");
        let graph = graph.shrink(crate::ir::ShrinkStrength::Aggressive);
        println!("Graph:\n{:?}", Dot::with_config(&graph, &[]));
        let s = Z3Backend::new(graph);
        let is_not_equal_drain = s.is_not_equal_drain_belt_balancer(&[24]);
        assert!(is_not_equal_drain);
    }

    #[test]
    fn is_equal_drain_belt_balancer() {
        let graph = load("tests/3-2-equal-drain");
        let graph = graph.shrink(crate::ir::ShrinkStrength::Aggressive);
        println!("Graph:\n{:?}", Dot::with_config(&graph, &[]));
        let s = Z3Backend::new(graph);
        let is_not_equal_drain = s.is_not_equal_drain_belt_balancer(&[24]);
        assert!(!is_not_equal_drain);
    }
}
