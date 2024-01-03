use petgraph::prelude::{EdgeIndex, NodeIndex};
use z3::Context;

use crate::ir::{Connector, Edge, FlowGraph, Input, Merger, Node, Output, Splitter};

use super::{
    model_entities::{kirchhoff_law, Z3Edge, Z3Node},
    model_graph::Z3QuantHelper,
};

pub trait Z3NodeRelaxed {
    fn model_relaxed<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    );
}

impl Z3NodeRelaxed for Node {
    fn model_relaxed<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    ) {
        match self {
            Self::Connector(c) => c.model_relaxed(graph, idx, ctx, helper),
            Self::Input(c) => c.model_relaxed(graph, idx, ctx, helper),
            Self::Output(c) => c.model_relaxed(graph, idx, ctx, helper),
            Self::Merger(c) => c.model_relaxed(graph, idx, ctx, helper),
            Self::Splitter(c) => c.model_relaxed(graph, idx, ctx, helper),
        }
    }
}

impl Z3NodeRelaxed for Connector {
    fn model_relaxed<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    ) {
        self.model(graph, idx, ctx, helper);
    }
}

impl Z3NodeRelaxed for Input {
    fn model_relaxed<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    ) {
        self.model(graph, idx, ctx, helper);
    }
}

impl Z3NodeRelaxed for Output {
    fn model_relaxed<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    ) {
        self.model(graph, idx, ctx, helper);
    }
}

impl Z3NodeRelaxed for Merger {
    fn model_relaxed<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    ) {
        self.model(graph, idx, ctx, helper);
    }
}

impl Z3NodeRelaxed for Splitter {
    fn model_relaxed<'a>(
        &self,
        graph: &FlowGraph,
        idx: NodeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    ) {
        kirchhoff_law(idx, graph, ctx, helper);
    }
}

pub trait Z3EdgeRelaxed {
    fn model_relaxed<'a>(
        &self,
        graph: &FlowGraph,
        idx: EdgeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    );
}

impl Z3EdgeRelaxed for Edge {
    fn model_relaxed<'a>(
        &self,
        graph: &FlowGraph,
        idx: EdgeIndex,
        ctx: &'a Context,
        helper: &mut Z3QuantHelper<'a>,
    ) {
        self.model(graph, idx, ctx, helper);
    }
}
