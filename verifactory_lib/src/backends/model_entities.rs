use fraction::GenericFraction;
use petgraph::{
    prelude::{EdgeIndex, NodeIndex},
    Direction::Outgoing,
};
use z3::ast::{Bool, Int, Real};

use crate::ir::{Connector, Edge, FlowGraph, GraphHelper, Input, Merger, Node, Output, Splitter};

use super::model_graph::{ModelFlags, Z3QuantHelper};

// TODO: document whole file
trait Z3Fraction {
    fn to_z3(&self) -> Real;
}

impl Z3Fraction for GenericFraction<u128> {
    fn to_z3(&self) -> Real {
        let num = *self.numer().unwrap() as i64;
        let den = *self.denom().unwrap() as i64;
        Real::from_rational(num, den)
    }
}
pub trait Z3Node {
    fn model(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        helper: &mut Z3QuantHelper,
        flags: ModelFlags,
    );
}

impl Z3Node for Node {
    fn model<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        helper: &mut Z3QuantHelper,
        flags: ModelFlags,
    ) {
        match self {
            Self::Connector(c) => c.model(graph, idx, helper, flags),
            Self::Input(c) => c.model(graph, idx, helper, flags),
            Self::Output(c) => c.model(graph, idx, helper, flags),
            Self::Merger(c) => c.model(graph, idx, helper, flags),
            Self::Splitter(c) => c.model(graph, idx, helper, flags),
        }
    }
}

pub fn kirchhoff_law<'a>(node_idx: NodeIndex, graph: &FlowGraph, helper: &mut Z3QuantHelper) {
    let edge_map = &helper.edge_map;
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

    let in_sum = Real::add(&in_consts);
    let out_sum = Real::add(&out_consts);

    let ast = in_sum.eq(&out_sum);
    helper.others.push(ast);
}

impl Z3Node for Connector {
    fn model<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        helper: &mut Z3QuantHelper,
        flags: ModelFlags,
    ) {
        kirchhoff_law(idx, graph, helper);

        if flags.contains(ModelFlags::Blocked) {
            // input blocked iff. output blocked
            let in_idx = graph.in_edge_idx(idx)[0];
            let out_idx = graph.out_edge_idx(idx)[0];
            let blocked_in = helper.blocked_edge_map.get(&in_idx).unwrap();
            let blocked_out = helper.blocked_edge_map.get(&out_idx).unwrap();

            let ast = blocked_in.iff(blocked_out);
            helper.blocking.push(ast);
        }
    }
}

impl Z3Node for Input {
    fn model<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        helper: &mut Z3QuantHelper,
        flags: ModelFlags,
    ) {
        /* create new input variable */
        let input_name = format!("input_{}", self.id);
        let input = Int::new_const(input_name);
        let input_real = Real::from_int(&input);
        helper.input_map.insert(idx, input);

        /* kirchhoff on input and out-edge */
        let out_idx = graph.out_edge_idx(idx)[0];
        let out = helper.edge_map.get(&out_idx).unwrap();

        let ast = input_real.eq(out);
        helper.others.push(ast);

        if flags.contains(ModelFlags::Blocked) {
            // add blocked variable to the map
            let out_idx = graph.out_edge_idx(idx)[0];
            helper.blocked_input_map.insert(
                idx,
                helper.blocked_edge_map.get(&out_idx).unwrap().to_owned(),
            );
        }
    }
}

impl Z3Node for Output {
    fn model<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        helper: &mut Z3QuantHelper,
        flags: ModelFlags,
    ) {
        /* create new output variable */
        let output_name = format!("output_{}", self.id);
        let output = Real::new_const(output_name);

        /* kirchhoff on output and in-edge */
        let in_idx = graph.in_edge_idx(idx)[0];
        let inp = helper.edge_map.get(&in_idx).unwrap();

        let ast = output.eq(inp);
        helper.others.push(ast);
        helper.output_map.insert(idx, output);

        if flags.contains(ModelFlags::Blocked) {
            // add blocked variable to the map
            let in_idx = graph.in_edge_idx(idx)[0];
            helper.blocked_output_map.insert(
                idx,
                helper.blocked_edge_map.get(&in_idx).unwrap().to_owned(),
            );
        }
    }
}

impl Z3Node for Merger {
    fn model<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        helper: &mut Z3QuantHelper,
        flags: ModelFlags,
    ) {
        kirchhoff_law(idx, graph, helper);

        if flags.contains(ModelFlags::Blocked) {
            // add `blocked` constraint to [`Merger`]
            let in_idx_1 = graph.in_edge_idx(idx)[0];
            let in_idx_2 = graph.in_edge_idx(idx)[1];
            let out_idx = graph.out_edge_idx(idx)[0];

            let blocked_in_1 = helper.blocked_edge_map.get(&in_idx_1).unwrap();
            let blocked_in_2 = helper.blocked_edge_map.get(&in_idx_2).unwrap();
            let blocked_out = helper.blocked_edge_map.get(&out_idx).unwrap();

            // if output is blocked, block both inputs
            // otherwise, don't block the inputs
            let ast = blocked_out.ite(
                &Bool::and(&[blocked_in_1, blocked_in_2]),
                &Bool::or(&[blocked_in_1, blocked_in_2]).not(),
            );
            helper.blocking.push(ast);
        }
    }
}

impl Z3Node for Splitter {
    fn model<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        helper: &mut Z3QuantHelper,
        flags: ModelFlags,
    ) {
        kirchhoff_law(idx, graph, helper);
        let splitter_cond = self.get_splitter_cond(graph, idx, helper);

        if flags.contains(ModelFlags::Relaxed) {
            // skip the splitter condition
        } else if flags.contains(ModelFlags::Blocked) {
            // add `blocked` constraint to [`Splitter`]
            let in_idx = graph.in_edge_idx(idx)[0];
            let out_idx_1 = graph.out_edge_idx(idx)[0];
            let out_idx_2 = graph.out_edge_idx(idx)[1];

            let blocked_in = helper.blocked_edge_map.get(&in_idx).unwrap();
            let blocked_out_1 = helper.blocked_edge_map.get(&out_idx_1).unwrap();
            let blocked_out_2 = helper.blocked_edge_map.get(&out_idx_2).unwrap();

            // remove splitter condition if at least one of the outputs is blocked
            let ast = Bool::or(&[blocked_out_1, blocked_out_2])
                .not()
                .implies(&splitter_cond);
            helper.others.push(ast);
            // if both outputs are blocked, block the input
            // otherwise, don't block the input
            let ast = Bool::and(&[blocked_out_1, blocked_out_2]).ite(blocked_in, &blocked_in.not());
            helper.blocking.push(ast);
        } else {
            // ModelFlags is empty (normal operation)
            helper.others.push(splitter_cond);
        }
    }
}

impl Splitter {
    pub fn get_splitter_cond<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        helper: &mut Z3QuantHelper,
    ) -> Bool {
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
            let min_cap_var = min_cap.to_z3();
            let out_min = min_cap * 2;
            let out_min_var = out_min.to_z3();

            in_var
                .le(&out_min_var)
                .ite(&min_var.eq(max_var), &min_var.eq(&min_cap_var))
        } else {
            let prio_idx = graph.get_edge(idx, Outgoing, side);
            let other_idx = graph.get_edge(idx, Outgoing, -side);

            let prio_var = helper.edge_map.get(&prio_idx).unwrap();
            let other_var = helper.edge_map.get(&other_idx).unwrap();

            let prio_cap = graph[prio_idx].capacity;
            let prio_cap_var = prio_cap.to_z3();
            let zero = Real::from_rational(0, 1);

            in_var
                .le(&prio_cap_var)
                .ite(&other_var.eq(&zero), &prio_var.eq(&prio_cap_var))
        }
    }
}

pub trait Z3Edge {
    fn model<'a>(
        &self,
        graph: &FlowGraph,
        idx: EdgeIndex,
        helper: &mut Z3QuantHelper,
        flags: ModelFlags,
    );
}

impl Z3Edge for Edge {
    fn model<'a>(
        &self,
        graph: &FlowGraph,
        idx: EdgeIndex,
        helper: &mut Z3QuantHelper,
        flags: ModelFlags,
    ) {
        let numer = *self.capacity.numer().unwrap() as i64;
        let denom = *self.capacity.denom().unwrap() as i64;
        let capacity = Real::from_rational(numer, denom);

        let (src, dst) = graph.edge_endpoints(idx).unwrap();
        let (src_id, dst_id) = (graph[src].get_str(), graph[dst].get_str());

        let edge_name = format!("edge_{}_{}_{}", src_id, dst_id, idx.index());
        let edge = Real::new_const(edge_name);
        let zero = Real::from_rational(0, 1);

        let ast = edge.le(&capacity);
        helper.others.push(ast);
        let ast = edge.ge(&zero);
        helper.others.push(ast);
        helper.edge_map.insert(idx, edge);

        // check if blocked
        if flags.contains(ModelFlags::Blocked) {
            // add `blocked` constraint to each edge in the model
            let edge = helper.edge_map.get(&idx).unwrap();
            let zero = Real::from_rational(0, 1);

            let (src, dst) = graph.edge_endpoints(idx).unwrap();
            let (src_id, dst_id) = (graph[src].get_str(), graph[dst].get_str());

            let blocked_name = format!("blocked_{}_{}_{}", src_id, dst_id, idx.index());
            let blocked = Bool::new_const(blocked_name);
            let blocked_capacity = blocked.implies(&edge.eq(&zero));

            helper.blocked_edge_map.insert(idx, blocked);

            // Maybe this should not be blocking but others?
            helper.others.push(blocked_capacity);
        }
    }
}
