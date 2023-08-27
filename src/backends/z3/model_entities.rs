use crate::ir::{Connector, Edge, Input, Merger, Node, Output, Side, Splitter};
use petgraph::{
    prelude::{EdgeIndex, NodeIndex},
    visit::EdgeRef,
    Direction::{Incoming, Outgoing},
};
use z3::ast::{Ast, Int, Real};

use super::{BuildHelper, Z3Backend};

pub trait Z3Node {
    fn model(&self, idx: NodeIndex, z3: &Z3Backend, helper: &mut BuildHelper);
}

pub trait Z3Edge {
    fn model(&self, idx: EdgeIndex, z3: &Z3Backend, helper: &mut BuildHelper);
}

impl Z3Node for Connector {
    fn model(&self, idx: NodeIndex, z3: &Z3Backend, helper: &mut BuildHelper) {
        let in_out_nodes = node_variables(idx, z3);
        kirchhoff_law(z3, helper, in_out_nodes);
    }
}

impl Z3Node for Input {
    fn model(&self, idx: NodeIndex, z3: &Z3Backend, helper: &mut BuildHelper) {
        let solver = &z3.solver;
        let ctx = solver.get_context();

        /* create new input variable */
        let input_name = format!("input{}", idx.index());
        let input = Int::new_const(ctx, input_name);
        let input_real = Real::from_int(&input);

        /* kirchhoff on input and outgoing edge */
        let out_idx = node_variables(idx, z3).1.next().unwrap();
        let out = helper.edge_map.get(&out_idx).unwrap();
        solver.assert(&input_real._eq(out));

        /* add the variable to the map */
        helper.input_map.insert(idx, input);
    }
}

impl Z3Node for Output {
    fn model(&self, idx: NodeIndex, z3: &Z3Backend, helper: &mut BuildHelper) {
        let solver = &z3.solver;
        let ctx = solver.get_context();

        /* create new output variable */
        let output_name = format!("output{}", idx.index());
        let output = Real::new_const(ctx, output_name);

        /* kirchhoff on input and outgoing edge */
        let in_idx = node_variables(idx, z3).0.next().unwrap();
        let input = helper.edge_map.get(&in_idx).unwrap();
        solver.assert(&output._eq(input));

        /* add the variable to the map */
        helper.output_map.insert(idx, output);
    }
}

fn get_priority_edge<'a>(
    side: Side,
    mut edges: impl Iterator<Item = (Option<Side>, &'a Real<'a>)>,
) -> &'a Real<'a> {
    edges
        .find(|(e, _)| matches!(e, Some(s) if *s == side))
        .unwrap()
        .1
}

fn compute_cost<'a>(
    z3: &Z3Backend,
    priority: Option<Side>,
    single: &'a Real<'a>,
    multi: impl Iterator<Item = (Option<Side>, &'a Real<'a>)>,
) -> Real<'a> {
    let ctx = z3.solver.get_context();
    let cost_calc = match priority {
        None => {
            let multi_edges = multi.map(|e| e.1).collect::<Vec<_>>();
            Real::sub(ctx, &multi_edges)
        }
        Some(side) => {
            let priority_edge = get_priority_edge(side, multi);
            Real::sub(ctx, &[single, priority_edge])
        }
    };
    let zero = Real::from_real(ctx, 0, 1);
    cost_calc
        .ge(&zero)
        .ite(&cost_calc, &cost_calc.unary_minus())
}

impl Z3Node for Merger {
    fn model(&self, idx: NodeIndex, z3: &Z3Backend, helper: &mut BuildHelper) {
        let solver = &z3.solver;
        let ctx = solver.get_context();

        /* create cost variable */
        let cost_name = format!("cost{}", idx.index());
        let cost = Real::new_const(ctx, cost_name);

        /* kirchhoff on input and outgoing edge */
        let mut in_out_nodes = node_variables(idx, z3);
        kirchhoff_law(z3, helper, in_out_nodes.clone());

        /* model priority */
        let input_sides = in_out_nodes
            .0
            .map(|idx| (z3.graph[idx].side, &helper.edge_map[&idx]));
        let output = &helper.edge_map[&in_out_nodes.1.next().unwrap()];

        let merger_priority = self.input_priority;
        let abs_cost = compute_cost(z3, merger_priority, output, input_sides);

        /* add the cost to the costs */
        solver.assert(&cost._eq(&abs_cost));
        helper.costs.push(cost);
    }
}

impl Z3Node for Splitter {
    fn model(&self, idx: NodeIndex, z3: &Z3Backend, helper: &mut BuildHelper) {
        let solver = &z3.solver;
        let ctx = solver.get_context();

        /* create cost variable */
        let cost_name = format!("cost{}", idx.index());
        let cost = Real::new_const(ctx, cost_name);

        /* kirchhoff on input and outgoing edge */
        let mut in_out_nodes = node_variables(idx, z3);
        kirchhoff_law(z3, helper, in_out_nodes.clone());

        /* model priority */
        let output_sides = in_out_nodes
            .1
            .map(|idx| (z3.graph[idx].side, &helper.edge_map[&idx]));
        let input = &helper.edge_map[&in_out_nodes.0.next().unwrap()];

        let splitter_priority = self.output_priority;
        let abs_cost = compute_cost(z3, splitter_priority, input, output_sides);

        /* add the cost to the costs */
        solver.assert(&cost._eq(&abs_cost));
        helper.costs.push(cost);
    }
}

fn node_variables(
    idx: NodeIndex,
    z3: &Z3Backend,
) -> (
    impl Iterator<Item = EdgeIndex> + '_ + Clone,
    impl Iterator<Item = EdgeIndex> + '_ + Clone,
) {
    let incoming = z3.graph.edges_directed(idx, Incoming).map(|e| e.id());
    let outgoing = z3.graph.edges_directed(idx, Outgoing).map(|e| e.id());
    (incoming, outgoing)
}

fn kirchhoff_law<'a>(
    z3: &Z3Backend,
    helper: &mut BuildHelper,
    nodes: (
        impl Iterator<Item = EdgeIndex> + 'a,
        impl Iterator<Item = EdgeIndex> + 'a,
    ),
) {
    let solver = &z3.solver;
    let ctx = solver.get_context();

    let (input, output) = nodes;
    let input = input.map(|e| helper.edge_map.get(&e).unwrap());
    let output = output.map(|e| helper.edge_map.get(&e).unwrap());

    let in_sum = Real::add(ctx, &input.collect::<Vec<_>>());
    let out_sum = Real::add(ctx, &output.collect::<Vec<_>>());

    solver.assert(&in_sum._eq(&out_sum));
}

impl Z3Node for Node {
    fn model(&self, idx: NodeIndex, z3: &Z3Backend, helper: &mut BuildHelper) {
        match self {
            Node::Connector(x) => x.model(idx, z3, helper),
            Node::Input(x) => x.model(idx, z3, helper),
            Node::Merger(x) => x.model(idx, z3, helper),
            Node::Output(x) => x.model(idx, z3, helper),
            Node::Splitter(x) => x.model(idx, z3, helper),
        }
    }
}

impl Z3Edge for Edge {
    fn model(&self, idx: EdgeIndex, z3: &Z3Backend, helper: &mut BuildHelper) {
        let solver = &z3.solver;
        let ctx = solver.get_context();

        let capacity = Real::from_real(ctx, self.capacity as i32, 1);
        let zero = Real::from_real(ctx, 0, 1);
        let edge_name = format!("edge_{}", idx.index());
        let edge = Real::new_const(ctx, edge_name);

        solver.assert(&edge.le(&capacity));
        solver.assert(&edge.ge(&zero));
        helper.edge_map.insert(idx, edge);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use petgraph::dot::Dot;
    use z3::SatResult;

    use crate::{
        compiler::Compiler,
        import::string_to_entities,
        ir::{FlowGraph, ShrinkStrength, Shrinkable},
    };

    use super::*;

    fn load(file: &str) -> FlowGraph {
        let blueprint_string = fs::read_to_string(file).unwrap();
        let entities = string_to_entities(&blueprint_string).unwrap();
        let graph = Compiler::new(entities).create_graph();
        println!("{:?}", Dot::with_config(&graph, &[]));
        graph
    }

    #[test]
    fn test_belt() {
        let g = load("tests/simple_belt");
        let s = Z3Backend::new(g);
        s.model();
        println!("{}", s.solver);
    }

    #[test]
    fn test_belt_shrink() {
        let g = load("tests/simple_belt");
        let g = g.shrink(ShrinkStrength::Aggressive);
        let s = Z3Backend::new(g);
        s.model();
        println!("{}", s.solver);
    }

    #[test]
    fn test_splitter() {
        let g = load("tests/simple_splitter");
        let s = Z3Backend::new(g);
        s.model();
        if s.solver.check(&[]) == SatResult::Sat {
            let model = s.solver.get_model().unwrap();
            println!("{}", model);
        } else {
            println!("UNSAT");
        }
    }
}
