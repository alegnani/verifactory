use std::collections::HashMap;

use fraction::GenericFraction;
use petgraph::{
    prelude::{EdgeIndex, NodeIndex},
    Direction::Outgoing,
};
use z3::{
    ast::{Ast, Int, Real},
    Context,
};

use crate::ir::{Connector, Edge, GraphHelper, Input, Merger, Node, Output, Splitter};

use super::model_graph::{Z3Backend, Z3Helper};

pub trait Z3Node {
    fn model<'a>(&self, idx: NodeIndex, z3: &'a Z3Backend, helper: Z3Helper<'a>) -> Z3Helper<'a>;
}

trait Z3Fraction {
    fn to_z3<'a>(&self, ctx: &'a Context) -> Real<'a>;
}

impl Z3Fraction for GenericFraction<u128> {
    fn to_z3<'a>(&self, ctx: &'a Context) -> Real<'a> {
        let num = *self.numer().unwrap() as i32;
        let den = *self.denom().unwrap() as i32;
        Real::from_real(ctx, num, den)
    }
}

pub trait Z3Edge {
    fn model<'a>(&self, idx: EdgeIndex, z3: &'a Z3Backend) -> Real<'a>;
}

impl Z3Node for Node {
    fn model<'a>(&self, idx: NodeIndex, z3: &'a Z3Backend, helper: Z3Helper<'a>) -> Z3Helper<'a> {
        match self {
            Node::Connector(x) => x.model(idx, z3, helper),
            Node::Input(x) => x.model(idx, z3, helper),
            Node::Merger(x) => x.model(idx, z3, helper),
            Node::Output(x) => x.model(idx, z3, helper),
            Node::Splitter(x) => x.model(idx, z3, helper),
        }
    }
}

fn kirchhoff_law(node_idx: NodeIndex, z3: &Z3Backend, edge_map: &HashMap<EdgeIndex, Real>) {
    let graph = z3.get_graph();
    let ctx = z3.get_ctx();
    let solver = z3.get_solver();

    let in_consts = graph
        .in_edge_idx(node_idx)
        .iter()
        .map(|idx| edge_map.get(idx).unwrap())
        .collect::<Vec<_>>();
    let out_consts = graph
        .out_edge_idx(node_idx)
        .iter()
        .map(|idx| edge_map.get(idx).unwrap())
        .collect::<Vec<_>>();

    let in_sum = Real::add(ctx, &in_consts);
    let out_sum = Real::add(ctx, &out_consts);

    solver.assert(&in_sum._eq(&out_sum));
}

impl Z3Node for Connector {
    fn model<'a>(&self, idx: NodeIndex, z3: &'a Z3Backend, helper: Z3Helper<'a>) -> Z3Helper<'a> {
        kirchhoff_law(idx, z3, &helper.edge_map);
        helper
    }
}

impl Z3Node for Merger {
    fn model<'a>(&self, idx: NodeIndex, z3: &'a Z3Backend, helper: Z3Helper<'a>) -> Z3Helper<'a> {
        kirchhoff_law(idx, z3, &helper.edge_map);
        helper
    }
}

impl Z3Node for Input {
    fn model<'a>(
        &self,
        idx: NodeIndex,
        z3: &'a Z3Backend,
        mut helper: Z3Helper<'a>,
    ) -> Z3Helper<'a> {
        let graph = z3.get_graph();
        let solver = z3.get_solver();
        let ctx = z3.get_ctx();

        /* create new input variable */
        let input_name = format!("input{}_{}", idx.index(), self.id);
        let input = Int::new_const(ctx, input_name);
        let input_real = Real::from_int(&input);

        /* kirchhoff on input and out-edge */
        let out_idx = graph.out_edge_idx(idx)[0];
        let out = helper.edge_map.get(&out_idx).unwrap();

        solver.assert(&input_real._eq(out));
        helper.input_map.insert(idx, input);
        helper
    }
}

impl Z3Node for Output {
    fn model<'a>(
        &self,
        idx: NodeIndex,
        z3: &'a Z3Backend,
        mut helper: Z3Helper<'a>,
    ) -> Z3Helper<'a> {
        let graph = z3.get_graph();
        let solver = z3.get_solver();
        let ctx = z3.get_ctx();

        /* create new output variable */
        let output_name = format!("output{}", idx.index());
        let output = Real::new_const(ctx, output_name);

        /* kirchhoff on output and in-edge */
        let in_idx = graph.in_edge_idx(idx)[0];
        let inp = helper.edge_map.get(&in_idx).unwrap();

        solver.assert(&output._eq(inp));
        helper.output_map.insert(idx, output);
        helper
    }
}

impl Z3Node for Splitter {
    fn model<'a>(&self, idx: NodeIndex, z3: &'a Z3Backend, helper: Z3Helper<'a>) -> Z3Helper<'a> {
        kirchhoff_law(idx, z3, &helper.edge_map);

        let graph = z3.get_graph();
        let solver = z3.get_solver();
        let ctx = z3.get_ctx();

        let in_idx = graph.in_edge_idx(idx)[0];
        let in_var = helper.edge_map.get(&in_idx).unwrap();

        let side = self.output_priority;
        if side.is_none() {
            let out_idxs = graph.out_edge_idx(idx);
            let a_idx = out_idxs[0];
            let b_idx = out_idxs[1];

            let a_cap = graph[a_idx].capacity;
            let b_cap = graph[b_idx].capacity;
            let (min_idx, max_idx) = if a_cap <= b_cap {
                (a_idx, b_idx)
            } else {
                (b_idx, a_idx)
            };

            let min_var = helper.edge_map.get(&min_idx).unwrap();
            let max_var = helper.edge_map.get(&max_idx).unwrap();

            let min_cap = graph[min_idx].capacity;
            let min_cap_var = min_cap.to_z3(ctx);
            let out_min = min_cap * 2;
            let out_min_var = out_min.to_z3(ctx);

            let splitter_const = in_var
                .le(&out_min_var)
                .ite(&min_var._eq(max_var), &min_var._eq(&min_cap_var));
            solver.assert(&splitter_const);
        } else {
            let prio_idx = graph.get_edge(idx, Outgoing, side);
            let other_idx = graph.get_edge(idx, Outgoing, -side);

            let prio_var = helper.edge_map.get(&prio_idx).unwrap();
            let other_var = helper.edge_map.get(&other_idx).unwrap();

            let prio_cap = graph[prio_idx].capacity;
            let prio_cap_var = prio_cap.to_z3(ctx);
            let zero = Real::from_real(ctx, 0, 1);

            let splitter_const = in_var
                .le(&prio_cap_var)
                .ite(&other_var._eq(&zero), &prio_var._eq(&prio_cap_var));
            solver.assert(&splitter_const);
        }
        helper
    }
}

impl Z3Edge for Edge {
    fn model<'a>(&self, idx: EdgeIndex, z3: &'a Z3Backend) -> Real<'a> {
        let solver = z3.get_solver();
        let ctx = z3.get_ctx();

        let numer = *self.capacity.numer().unwrap() as i32;
        let denom = *self.capacity.denom().unwrap() as i32;
        let capacity = Real::from_real(ctx, numer, denom);
        let zero = Real::from_real(ctx, 0, 1);
        let edge_name = format!("edge_{}", idx.index());
        let edge = Real::new_const(ctx, edge_name);

        solver.assert(&edge.le(&capacity));
        solver.assert(&edge.ge(&zero));
        edge
    }
}
