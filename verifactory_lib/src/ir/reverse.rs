//! Provides functionality to reverse the direction of a [`FlowGraph`].

use petgraph::Graph;

use super::{Connector, Edge, FlowGraph, Input, Merger, Node, Output, Splitter};
use crate::utils::Side;

/// Trait used to represent that something can be reversed in direction.
///
/// This is used to invert the direction of the [`FlowGraph`].
pub trait Reversable {
    fn reverse(&self) -> Self;
}

impl Reversable for Side {
    fn reverse(&self) -> Self {
        -*self
    }
}

impl Reversable for Edge {
    fn reverse(&self) -> Self {
        let mut rev = *self;
        rev.side = rev.side.reverse();
        rev
    }
}

impl Reversable for Node {
    fn reverse(&self) -> Self {
        match self {
            Node::Connector(c) => Node::Connector(Connector { ..*c }),
            Node::Input(i) => Node::Output(Output { id: i.id }),
            Node::Output(o) => Node::Input(Input { id: o.id }),
            Node::Merger(m) => Node::Splitter(Splitter {
                output_priority: m.input_priority.reverse(),
                id: m.id,
            }),
            Node::Splitter(s) => Node::Merger(Merger {
                input_priority: s.output_priority.reverse(),
                id: s.id,
            }),
        }
    }
}

impl Reversable for FlowGraph {
    fn reverse(&self) -> Self {
        let mut rev = self.clone();
        Graph::reverse(&mut rev);
        // Reverse edge side
        for edge in rev.edge_weights_mut() {
            edge.side = edge.side.reverse();
        }
        // Reverse contents of nodes
        for node in rev.node_weights_mut() {
            *node = node.reverse();
        }
        rev
    }
}

#[cfg(test)]
mod test {
    use crate::{
        frontend::Compiler,
        import::file_to_entities,
        ir::{CoalesceStrength::Aggressive, FlowGraphFun},
    };

    use super::*;

    #[test]
    fn reverse_3_2() {
        let entities = file_to_entities("tests/3-2").unwrap();
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[3], Aggressive);
        let rev = graph.reverse();
    }
}
