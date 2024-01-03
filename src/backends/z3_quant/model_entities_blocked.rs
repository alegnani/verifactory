use petgraph::prelude::{EdgeIndex, NodeIndex};
use z3::{
    ast::{Ast, Bool, Real},
    Context,
};

use crate::ir::{Connector, Edge, FlowGraph, GraphHelper, Input, Merger, Node, Output, Splitter};

use super::{
    model_entities::{kirchhoff_law, Z3Edge, Z3Node},
    model_graph::Z3QuantHelper,
};

pub trait Z3NodeBlocked {
    fn model_blocked<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    );
}

impl Z3NodeBlocked for Node {
    fn model_blocked<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    ) {
        match self {
            Self::Connector(c) => c.model_blocked(graph, idx, ctx, helper),
            Self::Input(c) => c.model_blocked(graph, idx, ctx, helper),
            Self::Output(c) => c.model_blocked(graph, idx, ctx, helper),
            Self::Merger(c) => c.model_blocked(graph, idx, ctx, helper),
            Self::Splitter(c) => c.model_blocked(graph, idx, ctx, helper),
        }
    }
}

impl Z3NodeBlocked for Connector {
    fn model_blocked<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    ) {
        self.model(graph, idx, ctx, helper);

        // input blocked iff. output blocked
        let in_idx = graph.in_edge_idx(idx)[0];
        let out_idx = graph.out_edge_idx(idx)[0];
        let blocked_in = helper.blocked_edge_map.get(&in_idx).unwrap();
        let blocked_out = helper.blocked_edge_map.get(&out_idx).unwrap();

        let ast = blocked_in.iff(blocked_out);
        helper.blocking.push(ast);
    }
}

impl Z3NodeBlocked for Input {
    fn model_blocked<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    ) {
        self.model(graph, idx, ctx, helper);

        // add blocked variable to the map
        let out_idx = graph.out_edge_idx(idx)[0];
        helper.blocked_input_map.insert(
            idx,
            helper.blocked_edge_map.get(&out_idx).unwrap().to_owned(),
        );
    }
}

impl Z3NodeBlocked for Output {
    fn model_blocked<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    ) {
        self.model(graph, idx, ctx, helper);

        // add blocked variable to the map
        let in_idx = graph.in_edge_idx(idx)[0];
        helper.blocked_output_map.insert(
            idx,
            helper.blocked_edge_map.get(&in_idx).unwrap().to_owned(),
        );
    }
}

impl Z3NodeBlocked for Merger {
    fn model_blocked<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    ) {
        self.model(graph, idx, ctx, helper);

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

impl Z3NodeBlocked for Splitter {
    fn model_blocked<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    ) {
        kirchhoff_law(idx, graph, ctx, helper);
        let splitter_cond = self.get_splitter_cond(graph, idx, ctx, helper);

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
    }
}

pub trait Z3EdgeBlocked {
    fn model_blocked<'a>(
        &self,
        graph: &FlowGraph,
        idx: EdgeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    );
}

impl Z3EdgeBlocked for Edge {
    fn model_blocked<'a>(
        &self,
        graph: &FlowGraph,
        idx: EdgeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    ) {
        self.model(graph, idx, ctx, helper);

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
