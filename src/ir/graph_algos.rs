use crate::{entities::EntityId, ir::Lattice};

use super::{FlowGraph, Node};
use petgraph::{
    prelude::NodeIndex,
    visit::EdgeRef,
    Direction::{Incoming, Outgoing},
};

/// Indicates how much a graph is shrunk.
/// Shrinking is performed on a Connector S, where A->S->B, with in_deg(S) = out_deg(S) = 1.
/// The result of the shrinking operation is A->B, with the edge having the minimum of the capacities of the previous two edges.
/// Shrinking also removes connectors with a missing in- or out-edge, e.g. after removing an input or output node.
/// Following this also splitters and mergers with only one out- and in-edge, respectively, get optimized away.
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
    fn remove_entities(self, exclude_list: &[EntityId]) -> Self;
}

trait ShrinkableHelper {
    fn is_valid_shrink(&self, idx: NodeIndex) -> bool;
}

impl ShrinkableHelper for FlowGraph {
    fn is_valid_shrink(&self, idx: NodeIndex) -> bool {
        self.node_weight(idx)
            .map(|n| matches!(n, Node::Connector(_) | Node::Input(_) | Node::Output(_)))
            .unwrap_or(false)
    }
}

impl Shrinkable for FlowGraph {
    fn shrink(mut self, strength: ShrinkStrength) -> Self {
        'outer: loop {
            for node_idx in self.node_indices() {
                let in_edges = self.edges_directed(node_idx, Incoming).collect::<Vec<_>>();
                let out_edges = self.edges_directed(node_idx, Outgoing).collect::<Vec<_>>();
                let in_deg = in_edges.len();
                let out_deg = out_edges.len();
                let node = &self[node_idx];

                /* ignore inputs and outputs */
                if matches!(node, Node::Input(_) | Node::Output(_)) {
                    continue;
                }

                if in_deg == 0 || out_deg == 0 {
                    self.remove_node(node_idx);
                    continue 'outer;
                }
                let source_node = in_edges[0].source();
                let target_node = out_edges[0].target();

                let should_join = match node {
                    Node::Connector(_) => {
                        /* only connectors with in_deg = out_deg = 1 can be shrunk.
                         * only shrink connectors between connectors, inputs or outputs. */
                        if !(self.is_valid_shrink(source_node) && self.is_valid_shrink(target_node))
                        {
                            continue;
                        }
                        /* check for the ShrinkStrength */
                        if let ShrinkStrength::Lossless = strength {
                            let source_id = self[source_node].get_id();
                            let target_id = self[target_node].get_id();
                            let id = self[node_idx].get_id();
                            if !(source_id == id || id == target_id) {
                                continue;
                            }
                        }
                        true
                    }
                    Node::Merger(_) => {
                        /* can only remove if in_deg == 1 */
                        if in_deg == 2 {
                            continue;
                        }
                        false
                    }
                    Node::Splitter(_) => {
                        /* can only remove if out_deg == 1 */
                        if out_deg == 2 {
                            continue;
                        }
                        false
                    }
                    _ => continue,
                };
                let in_edge = in_edges[0].weight();
                let out_edge = out_edges[0].weight();
                /* When shrinking connectors use join in order to preserve
                 * side information for splitters/mergers.
                 * When shrinking mergers/splitters we can safely loose this
                 * information (or at least in the case of no inserters and no
                 * 2-sided belts). */
                let new_edge = if should_join {
                    in_edge.join(out_edge)
                } else {
                    in_edge.meet(out_edge)
                };
                self.add_edge(source_node, target_node, new_edge);
                self.remove_node(node_idx);
                continue 'outer;
            }
            return self;
        }
    }

    fn remove_entities(mut self, exclude_list: &[EntityId]) -> Self {
        'outer: loop {
            for node_idx in self.node_indices() {
                let node = &self[node_idx];
                /* can only remove inputs or outputs */
                if !matches!(node, Node::Input(_) | Node::Output(_)) {
                    continue;
                }
                if exclude_list.contains(&node.get_id()) {
                    self.remove_node(node_idx);
                    continue 'outer;
                }
            }
            return self;
        }
    }
}

#[cfg(test)]
mod test {
    use petgraph::dot::Dot;

    use crate::{compiler::Compiler, entities::Entity, import::string_to_entities};

    use super::*;

    use std::fs;

    fn load(file: &str) -> Vec<Entity<i32>> {
        let blueprint_string = fs::read_to_string(file).unwrap();
        string_to_entities(&blueprint_string).unwrap()
    }
    #[test]
    fn test_shrinking() {
        let entities = load("tests/3-2-broken");
        let graph = Compiler::new(entities)
            .create_graph()
            .remove_entities(&[4, 5, 6])
            .shrink(ShrinkStrength::Aggressive);
        println!("{:?}", Dot::with_config(&graph, &[]));
    }
}
