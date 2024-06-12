use std::{cmp::Ordering, fs::File, io::Write};

use crate::entities::EntityId;

use super::{Connector, FlowGraph, GraphHelper, Lattice, Node};
use graphviz_rust::{cmd::Format, exec_dot};
use petgraph::{dot::Dot, prelude::EdgeIndex, Direction::Outgoing};

/// Indicates how much a graph is coalesced.
/// Coalescing is performed on a Connector S, where A->S->B, with in_deg(S) = out_deg(S) = 1.
/// The result of the coalescing operation is A->B, with the edge having the minimum of the capacities of the previous two edges.
/// Coalescing also removes connectors with a missing in- or out-edge, e.g. after removing an input or output node.
/// Following this also splitters and mergers with only one out- and in-edge, respectively, get optimized away.
#[derive(Clone, Copy)]
pub enum CoalesceStrength {
    /// Coalescing without loss of information about the structure of the blueprint.
    /// Coalesced only if:
    /// A, B are {Connector, Input, Output}
    /// A.id = S.id or S.id = B.id
    Lossless,
    /// Coalescing with loss of information for minimum size.
    /// Coalesced only if:
    /// A, B are {Connector, Input, Output}
    Aggressive,
}

// TODO: docs
/// Trait to define helper functions for graph simplification on a [`petgraph::Graph`] type.
trait FlowGraphSimplify {
    /// Merges superfluous nodes given a [`CoalesceStrength`] strategy.
    ///
    /// Returns `true` after the first node has been mutated.
    /// Otherwise, if no node has been mutated, return `false`.
    fn coalesce_nodes(&mut self, strength: CoalesceStrength) -> bool;
    /// Shrinks the capacities of all edges to their minimum bound.
    ///
    /// Returns `true` after the first edge has been mutated.
    /// Otherwise, if no edge has been mutated, return `false`.
    fn shrink_capacities(&mut self) -> bool;
    /// Removes the inputs and outputs from the graph associated with the provided [`EntityId`]s.
    fn remove_false_io(&mut self, exclude_list: &[EntityId]);
}

/// Trait to define how different nodes in the IR graph are shrunk.
trait ShrinkNodes {
    fn shrink_capacity_connector(&mut self, in_idx: EdgeIndex, out_idx: EdgeIndex) -> bool;
    fn shrink_capacity_splitter_no_prio(
        &mut self,
        in_idx: EdgeIndex,
        prio_idx: EdgeIndex,
        other_idx: EdgeIndex,
    ) -> bool;
    fn shrink_capacity_splitter_prio(
        &mut self,
        in_idx: EdgeIndex,
        a_idx: EdgeIndex,
        b_idx: EdgeIndex,
    ) -> bool;
    fn shrink_capacity_merger(
        &mut self,
        out_idx: EdgeIndex,
        a_idx: EdgeIndex,
        b_idx: EdgeIndex,
    ) -> bool;
}

/// Trait exposing the simplification and exporting of the IR graph
pub trait FlowGraphFun {
    fn simplify(&mut self, exclude_list: &[EntityId], strength: CoalesceStrength);
    fn to_png(&self, path: &str) -> anyhow::Result<()>;
}

impl FlowGraphFun for FlowGraph {
    fn simplify(&mut self, exclude_list: &[EntityId], strength: CoalesceStrength) {
        self.remove_false_io(exclude_list);
        loop {
            if self.coalesce_nodes(strength) {
                continue;
            }

            if self.shrink_capacities() {
                continue;
            }
            return;
        }
    }

    fn to_png(&self, path: &str) -> anyhow::Result<()> {
        let png = exec_dot(
            format!("{:?}", Dot::with_config(self, &[])),
            vec![Format::Png.into()],
        )?;
        File::create(path)?.write_all(&png)?;
        Ok(())
    }
}

impl FlowGraphSimplify for FlowGraph {
    fn coalesce_nodes(&mut self, strength: CoalesceStrength) -> bool {
        for node_idx in self.node_indices() {
            let in_deg = self.in_deg(node_idx);
            let out_deg = self.out_deg(node_idx);
            let node = &self[node_idx];

            /* ignore inputs and outputs */
            if matches!(node, Node::Input(_) | Node::Output(_)) {
                if in_deg == 0 && out_deg == 0 {
                    self.remove_node(node_idx);
                    return true;
                }
                continue;
            }

            if in_deg == 0 || out_deg == 0 {
                self.remove_node(node_idx);
                return true;
            }
            let source_node = self.in_nodes(node_idx)[0];
            let target_node = self.out_nodes(node_idx)[0];

            match node {
                Node::Connector(_) => {
                    /* don't coalesce a node that is between a splitter and a merger (S -> N -> M)
                     * as this would break the edge side field */
                    // if matches!(self[source_node], Node::Splitter(_))
                    //     && matches!(self[target_node], Node::Merger(_))
                    // {
                    //     continue;
                    // }
                    if matches!(self[source_node], Node::Splitter(_) | Node::Merger(_))
                        && matches!(self[target_node], Node::Merger(_) | Node::Splitter(_))
                    {
                        continue;
                    }
                    /* check for the ShrinkStrength */
                    if let CoalesceStrength::Lossless = strength {
                        let source_id = self[source_node].get_id();
                        let target_id = self[target_node].get_id();
                        let id = self[node_idx].get_id();
                        if !(source_id == id || id == target_id) {
                            continue;
                        }
                    }
                }
                Node::Merger(_) | Node::Splitter(_) => {
                    // skip if fully populated
                    if in_deg + out_deg == 3 {
                        continue;
                    }
                    // substitue a merger/splitter with a connector as it must have in_deg = 1 and out_deg = 1
                    self[node_idx] = Node::Connector(Connector { id: node.get_id() });
                    return true;
                }
                _ => continue,
            }
            let in_edge = self.in_edges(node_idx)[0];
            let out_edge = self.out_edges(node_idx)[0];
            /*
             * When shrinking connectors use join in order to preserve
             * side information for splitters/mergers.
             * Skip merging two edges if they can't be joined without ambiguity.
             */
            if in_edge.can_join(out_edge) {
                let new_edge = in_edge.join(out_edge);
                self.add_edge(source_node, target_node, new_edge);
                self.remove_node(node_idx);
                return true;
            }
        }
        false
    }

    fn remove_false_io(&mut self, exclude_list: &[EntityId]) {
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
            return;
        }
    }

    fn shrink_capacities(&mut self) -> bool {
        for node_idx in self.node_indices() {
            let node = &self[node_idx];
            let changed = match node {
                Node::Connector(_) => {
                    let in_idx = self.in_edge_idx(node_idx)[0];
                    let out_idx = self.out_edge_idx(node_idx)[0];
                    self.shrink_capacity_connector(in_idx, out_idx)
                }
                Node::Splitter(s) => {
                    let in_idx = self.in_edge_idx(node_idx)[0];
                    if s.output_priority.is_none() {
                        let out_idxs = self.out_edge_idx(node_idx);
                        self.shrink_capacity_splitter_no_prio(in_idx, out_idxs[0], out_idxs[1])
                    } else {
                        let prio_idx = self.get_edge(node_idx, Outgoing, s.output_priority);
                        let other_idx = self.get_edge(node_idx, Outgoing, -s.output_priority);
                        self.shrink_capacity_splitter_prio(in_idx, prio_idx, other_idx)
                    }
                }
                Node::Merger(_) => {
                    let out_idx = self.out_edge_idx(node_idx)[0];
                    let in_idxs = self.in_edge_idx(node_idx);
                    self.shrink_capacity_merger(out_idx, in_idxs[0], in_idxs[1])
                }
                _ => false,
            };
            if changed {
                return true;
            }
        }
        false
    }
}

impl ShrinkNodes for FlowGraph {
    fn shrink_capacity_splitter_prio(
        &mut self,
        in_idx: EdgeIndex,
        prio_idx: EdgeIndex,
        other_idx: EdgeIndex,
    ) -> bool {
        let prio_cap = self[prio_idx].capacity;
        let other_cap = self[other_idx].capacity;
        let in_cap = self[in_idx].capacity;
        let out_cap = prio_cap + other_cap;

        let (new_in, new_prio, new_other) = if out_cap == in_cap {
            (in_cap, prio_cap, other_cap)
        } else if out_cap < in_cap {
            (out_cap, prio_cap, other_cap)
        } else if prio_cap >= in_cap {
            (in_cap, in_cap, 0.into())
        } else {
            (in_cap, prio_cap, in_cap - prio_cap)
        };

        self[in_idx].capacity = new_in;
        self[prio_idx].capacity = new_prio;
        self[other_idx].capacity = new_other;

        (in_cap, prio_cap, other_cap) != (new_in, new_prio, new_other)
    }

    fn shrink_capacity_splitter_no_prio(
        &mut self,
        in_idx: EdgeIndex,
        a_idx: EdgeIndex,
        b_idx: EdgeIndex,
    ) -> bool {
        let a_cap = self[a_idx].capacity;
        let b_cap = self[b_idx].capacity;
        let in_cap = self[in_idx].capacity;
        let out_cap = a_cap + b_cap;

        let (new_in, new_a, new_b) = match out_cap.cmp(&in_cap) {
            Ordering::Equal => (in_cap, a_cap, b_cap),
            Ordering::Less => (out_cap, a_cap, b_cap),
            Ordering::Greater => {
                let half_in = in_cap / 2.;
                let a_big = a_cap > half_in;
                let b_big = b_cap > half_in;
                match (a_big, b_big) {
                    (true, true) => (in_cap, half_in, half_in),
                    (true, _) => (in_cap, in_cap - b_cap, b_cap),
                    (_, true) => (in_cap, a_cap, in_cap - a_cap),
                    _ => panic!(),
                }
            }
        };

        self[in_idx].capacity = new_in;
        self[a_idx].capacity = new_a;
        self[b_idx].capacity = new_b;

        (in_cap, a_cap, b_cap) != (new_in, new_a, new_b)
    }

    fn shrink_capacity_connector(&mut self, in_idx: EdgeIndex, out_idx: EdgeIndex) -> bool {
        let in_cap = self[in_idx].capacity;
        let out_cap = self[out_idx].capacity;

        if in_cap != out_cap {
            let min = in_cap.min(out_cap);
            self[in_idx].capacity = min;
            self[out_idx].capacity = min;
            true
        } else {
            false
        }
    }

    fn shrink_capacity_merger(
        &mut self,
        out_idx: EdgeIndex,
        a_idx: EdgeIndex,
        b_idx: EdgeIndex,
    ) -> bool {
        let out_cap = self[out_idx].capacity;
        let a_cap = self[a_idx].capacity;
        let b_cap = self[b_idx].capacity;
        let in_cap = a_cap + b_cap;

        let (new_out, new_a, new_b) = match in_cap.cmp(&out_cap) {
            Ordering::Equal => (out_cap, a_cap, b_cap),
            /* FIXME: this is just an ugly fix */
            // Ordering::Less => (out_cap.min(a_cap + b_cap), a_cap, b_cap),
            Ordering::Less => (out_cap, a_cap, b_cap),
            Ordering::Greater => (out_cap, a_cap.min(out_cap), b_cap.min(out_cap)),
        };

        self[out_idx].capacity = new_out;
        self[a_idx].capacity = new_a;
        self[b_idx].capacity = new_b;

        (new_out, new_a, new_b) != (out_cap, a_cap, b_cap)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        frontend::Compiler,
        import::file_to_entities,
        ir::{graph_algos::FlowGraphSimplify, CoalesceStrength::Aggressive, FlowGraphFun},
    };

    #[test]
    fn test_shrinking() {
        let entities = file_to_entities("tests/3-2-broken").unwrap();
        let mut graph = Compiler::new(entities).create_graph();
        graph.remove_false_io(&[]);
        graph.simplify(&[4, 5, 6], Aggressive);
        assert_eq!(graph.node_count(), 10);
        assert_eq!(graph.edge_count(), 9);
    }

    #[test]
    fn belt_reduction() {
        let entities = file_to_entities("tests/belt_reduction").unwrap();
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[], Aggressive);
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
        assert_eq!(graph.edge_weights().next().unwrap().capacity, 15.into());
    }

    #[test]
    fn splitter_reduction() {
        let entities = file_to_entities("tests/splitter_reduction").unwrap();
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[4], Aggressive);
        assert_eq!(graph.node_count(), 4);
        assert_eq!(graph.edge_count(), 3);
    }

    #[test]
    fn splitter_merger_reduction() {
        let entities = file_to_entities("tests/splitter_merger_reduction").unwrap();
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[4, 5], Aggressive);
        assert_eq!(graph.node_count(), 16);
        assert_eq!(graph.edge_count(), 16);
    }

    #[test]
    fn prio_splitter() {
        let entities = file_to_entities("tests/prio_splitter").unwrap();
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[], Aggressive);
    }
}
