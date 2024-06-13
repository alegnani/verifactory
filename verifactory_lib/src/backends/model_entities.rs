use fraction::GenericFraction;
use petgraph::{
    prelude::{EdgeIndex, NodeIndex},
    Direction::Outgoing,
};
use z3::{
    ast::{Ast, Bool, Int, Real},
    Context,
};

use crate::ir::{Connector, Edge, FlowGraph, GraphHelper, Input, Merger, Node, Output, Splitter};

use super::model_graph::{ModelFlags, Z3QuantHelper};

// TODO: document whole file
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
pub trait Z3Node {
    fn model<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
        flags: ModelFlags,
    );
}

impl Z3Node for Node {
    fn model<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
        flags: ModelFlags,
    ) {
        match self {
            Self::Connector(c) => c.model(graph, idx, ctx, helper, flags),
            Self::Input(c) => c.model(graph, idx, ctx, helper, flags),
            Self::Output(c) => c.model(graph, idx, ctx, helper, flags),
            Self::Merger(c) => c.model(graph, idx, ctx, helper, flags),
            Self::Splitter(c) => c.model(graph, idx, ctx, helper, flags),
        }
    }
}

pub fn kirchhoff_law<'a>(
    node_idx: NodeIndex,
    graph: &FlowGraph,
    ctx: &'a Context,
    helper: &mut Z3QuantHelper<'a>,
) {
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

    let in_sum = Real::add(ctx, &in_consts);
    let out_sum = Real::add(ctx, &out_consts);

    let ast = in_sum._eq(&out_sum);
    helper.others.push(ast);
}

impl Z3Node for Connector {
    fn model<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
        flags: ModelFlags,
    ) {
        kirchhoff_law(idx, graph, ctx, helper);

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
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
        flags: ModelFlags,
    ) {
        /* create new input variable */
        let input_name = format!("input_{}", self.id);
        let input = Real::new_const(ctx, input_name);
        helper.input_map.insert(idx, input.clone());

        /* kirchhoff on input and out-edge */
        let out_idx = graph.out_edge_idx(idx)[0];
        let out = helper.edge_map.get(&out_idx).unwrap();

        let ast = input._eq(out);
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
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
        flags: ModelFlags,
    ) {
        /* create new output variable */
        let output_name = format!("output_{}", self.id);
        let output = Real::new_const(ctx, output_name);

        /* kirchhoff on output and in-edge */
        let in_idx = graph.in_edge_idx(idx)[0];
        let inp = helper.edge_map.get(&in_idx).unwrap();

        let ast = output._eq(inp);
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
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
        flags: ModelFlags,
    ) {
        kirchhoff_law(idx, graph, ctx, helper);

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
                &Bool::and(ctx, &[blocked_in_1, blocked_in_2]),
                &Bool::or(ctx, &[blocked_in_1, blocked_in_2]).not(),
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
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
        flags: ModelFlags,
    ) {
        kirchhoff_law(idx, graph, ctx, helper);
        let splitter_cond = self.get_splitter_cond(graph, idx, ctx, helper);

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
            let ast = Bool::or(ctx, &[blocked_out_1, blocked_out_2])
                .not()
                .implies(&splitter_cond);
            helper.others.push(ast);
            // if both outputs are blocked, block the input
            // otherwise, don't block the input
            let ast =
                Bool::and(ctx, &[blocked_out_1, blocked_out_2]).ite(blocked_in, &blocked_in.not());
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
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    ) -> Bool<'a> {
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

            in_var
                .le(&out_min_var)
                .ite(&min_var._eq(max_var), &min_var._eq(&min_cap_var))
        } else {
            let prio_idx = graph.get_edge(idx, Outgoing, side);
            let other_idx = graph.get_edge(idx, Outgoing, -side);

            let prio_var = helper.edge_map.get(&prio_idx).unwrap();
            let other_var = helper.edge_map.get(&other_idx).unwrap();

            let prio_cap = graph[prio_idx].capacity;
            let prio_cap_var = prio_cap.to_z3(ctx);
            let zero = Real::from_real(ctx, 0, 1);

            in_var
                .le(&prio_cap_var)
                .ite(&other_var._eq(&zero), &prio_var._eq(&prio_cap_var))
        }
    }
}

pub trait Z3Edge {
    fn model<'a>(
        &self,
        graph: &FlowGraph,
        idx: EdgeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
        flags: ModelFlags,
    );
}

impl Z3Edge for Edge {
    fn model<'a>(
        &self,
        graph: &FlowGraph,
        idx: EdgeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
        flags: ModelFlags,
    ) {
        let numer = *self.capacity.numer().unwrap() as i32;
        let denom = *self.capacity.denom().unwrap() as i32;
        let capacity = Real::from_real(ctx, numer, denom);

        let (src, dst) = graph.edge_endpoints(idx).unwrap();
        let (src_id, dst_id) = (graph[src].get_str(), graph[dst].get_str());

        let edge_name = format!("edge_{}_{}_{}", src_id, dst_id, idx.index());
        let edge = Real::new_const(ctx, edge_name);
        let zero = Real::from_real(ctx, 0, 1);

        let ast = edge.le(&capacity);
        helper.others.push(ast);
        let ast = edge.ge(&zero);
        helper.others.push(ast);
        helper.edge_map.insert(idx, edge);

        // check if blocked
        if flags.contains(ModelFlags::Blocked) {
            // add `blocked` constraint to each edge in the model
            let edge = helper.edge_map.get(&idx).unwrap();
            let zero = Real::from_real(ctx, 0, 1);

            let (src, dst) = graph.edge_endpoints(idx).unwrap();
            let (src_id, dst_id) = (graph[src].get_str(), graph[dst].get_str());

            let blocked_name = format!("blocked_{}_{}_{}", src_id, dst_id, idx.index());
            let blocked = Bool::new_const(ctx, blocked_name);
            let blocked_capacity = blocked.implies(&edge._eq(&zero));

            helper.blocked_edge_map.insert(idx, blocked);

            // Maybe this should not be blocking but others?
            helper.others.push(blocked_capacity);
        }
    }
}
