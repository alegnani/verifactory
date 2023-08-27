use super::{Edge, FlowGraph, Node};
use petgraph::{
    prelude::NodeIndex,
    Direction::{Incoming, Outgoing},
};

/// Indicates how much a graph is shrunk.
/// Shrinking is performed on a Connector S, where A->S->B, with in_deg(S) = out_deg(S) = 1.
/// The result of the shrinking operation is A->B, with the edge having the minimum of the capacities of the previous two edges.
pub enum ShrinkStrength {
    /// Shrinking without loss of information about the structure of the blueprint.
    /// Shrinks only if:
    /// A, B are {Connector, Input, Output}
    /// A.id = S.id or S.id = B.id
    Lossless,
    /// Shrinking with loss of information for minimum size.
    /// Shrinks only if:
    /// A, B are {Connector, Input, Output}
    Aggressive,
}
pub trait Shrinkable {
    fn shrink(self, strength: ShrinkStrength) -> Self;
}

impl Shrinkable for FlowGraph {
    fn shrink(mut self, strength: ShrinkStrength) -> Self {
        'outer: loop {
            for node in self.node_indices() {
                if let Some(Node::Connector(_)) = self.node_weight(node) {
                    let mut in_edges = self.edges_directed(node, Incoming);
                    let mut out_edges = self.edges_directed(node, Outgoing);

                    /* only connectors with in_deg = out_deg = 1 can be shrunk */
                    let in_degree = in_edges.clone().count();
                    let out_degree = out_edges.clone().count();
                    if out_degree != 1 || in_degree != 1 {
                        continue;
                    }

                    /* only shrink connectors between connectors, inputs or outputs */
                    let source_node = self.neighbors_directed(node, Incoming).next().unwrap();
                    let dest_node = self.neighbors_directed(node, Outgoing).next().unwrap();
                    if !(is_valid_shrink(source_node, &self) || is_valid_shrink(dest_node, &self)) {
                        continue;
                    }
                    /* check for the ShrinkStrength */
                    if let ShrinkStrength::Lossless = strength {
                        let source_id = self.node_weight(source_node).unwrap().get_id();
                        let dest_id = self.node_weight(dest_node).unwrap().get_id();
                        let id = self.node_weight(node).unwrap().get_id();
                        if !(source_id == id || id == dest_id) {
                            continue;
                        }
                    }

                    let in_node = in_edges.next().unwrap();
                    let out_node = out_edges.next().unwrap();
                    let new_cap = in_node.weight().capacity.min(out_node.weight().capacity);
                    let new_edge = Edge {
                        capacity: new_cap,
                        side: None,
                    };

                    self.add_edge(source_node, dest_node, new_edge);
                    self.remove_node(node);
                    continue 'outer;
                }
            }
            return self;
        }
    }
}

fn is_valid_shrink(idx: NodeIndex, graph: &FlowGraph) -> bool {
    graph
        .node_weight(idx)
        .map(|n| matches!(n, Node::Connector(_) | Node::Input(_) | Node::Output(_)))
        .unwrap_or(false)
}
