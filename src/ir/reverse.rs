use petgraph::Graph;

use super::{Connector, Edge, FlowGraph, Input, Merger, Node, Output, Side, Splitter};

pub trait Reversable {
    fn reverse(&self) -> Self;
}

impl Reversable for Side {
    fn reverse(&self) -> Self {
        match self {
            Self::Right => Self::Left,
            Self::Left => Self::Right,
        }
    }
}

impl Reversable for Option<Side> {
    fn reverse(&self) -> Self {
        self.map(|s| s.reverse())
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
            Node::Connector(c) => Node::Connector(Connector { id: c.id }),
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
        /* Reverse edge side */
        for edge in rev.edge_weights_mut() {
            if let Some(side) = edge.side {
                edge.side = Some(match side {
                    Side::Left => Side::Right,
                    Side::Right => Side::Left,
                });
            }
        }
        for node in rev.node_weights_mut() {
            *node = node.reverse();
        }
        rev
    }
}

#[cfg(test)]
mod test {
    use crate::{compiler::Compiler, import::file_to_entities, ir::FlowGraphFun};

    use super::*;

    #[test]
    fn reverse_3_2() {
        let entities = file_to_entities("tests/3-2").unwrap();
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[3]);
        graph.to_svg("tests/3-2-normal.svg").unwrap();
        let rev = graph.reverse();
        rev.to_svg("tests/3-2-rev.svg").unwrap();
    }
}
