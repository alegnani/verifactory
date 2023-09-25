use std::{fs::File, io::Write};

use crate::{entities::EntityId, ir::Lattice};

use super::{FlowGraph, GraphHelper, Node};
use graphviz_rust::{cmd::Format, exec_dot};
use petgraph::{
    dot::Dot,
    prelude::{EdgeIndex, NodeIndex},
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
    fn coalesce_nodes(&mut self, strength: ShrinkStrength) -> bool;
    fn shrink_capacities(&mut self) -> bool;
    fn remove_false_io(&mut self, exclude_list: &[EntityId]);
    fn simplify(&mut self, exclude_list: &[EntityId]);
}

pub trait Svg {
    fn to_svg(&self, path: &str) -> anyhow::Result<()>;
}

impl Svg for FlowGraph {
    fn to_svg(&self, path: &str) -> anyhow::Result<()> {
        let svg = exec_dot(
            format!("{:?}", Dot::with_config(self, &[])),
            vec![Format::Svg.into()],
        )?;
        File::create(path)?.write_all(svg.as_bytes())?;
        Ok(())
    }
}

impl Shrinkable for FlowGraph {
    fn coalesce_nodes(&mut self, strength: ShrinkStrength) -> bool {
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

            let should_join = match node {
                Node::Connector(_) => {
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
            let in_edge = self.in_edges(node_idx)[0];
            let out_edge = self.out_edges(node_idx)[0];
            /* When shrinking connectors use join in order to preserve
             * side information for splitters/mergers.
             * When shrinking mergers/splitters we can safely lose this
             * information (or at least in the case of no inserters and no
             * 2-sided belts). */
            let new_edge = if should_join {
                in_edge.join(out_edge)
            } else {
                in_edge.meet(out_edge)
            };
            self.add_edge(source_node, target_node, new_edge);
            self.remove_node(node_idx);
            return true;
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

    fn simplify(&mut self, exclude_list: &[EntityId]) {
        self.remove_false_io(exclude_list);
        loop {
            self.to_svg("foo.svg").unwrap();
            if self.coalesce_nodes(ShrinkStrength::Aggressive) {
                continue;
            }

            if self.shrink_capacities() {
                continue;
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
                    match s.output_priority {
                        None => {
                            let out_idxs = self.out_edge_idx(node_idx);
                            self.shrink_capacity_splitter_no_prio(in_idx, out_idxs[0], out_idxs[1])
                        }
                        Some(priority) => {
                            let prio_idx = self.get_edge(node_idx, Outgoing, priority);
                            let other_idx = self.get_edge(node_idx, Outgoing, priority.other());
                            self.shrink_capacity_splitter_prio(in_idx, prio_idx, other_idx)
                        }
                    }
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

pub trait ShrinkNodes {
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
    fn shrink_capacity_merger_prio(
        &mut self,
        out_idx: EdgeIndex,
        prio_idx: EdgeIndex,
        other_idx: EdgeIndex,
    ) -> bool;
    fn shrink_capacity_merger_no_prio(
        &mut self,
        out_idx: EdgeIndex,
        a_idx: EdgeIndex,
        b_idx: EdgeIndex,
    ) -> bool;
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
            (in_cap, in_cap, 0.)
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

        let (new_in, new_a, new_b) = if out_cap == in_cap {
            (in_cap, a_cap, b_cap)
        } else if out_cap < in_cap {
            (out_cap, a_cap, b_cap)
        } else {
            let half_in = in_cap / 2.;
            let a_big = a_cap > half_in;
            let b_big = b_cap > half_in;
            if a_big && b_big {
                (in_cap, half_in, half_in)
            } else if a_big {
                (in_cap, in_cap - b_cap, b_cap)
            } else if b_big {
                (in_cap, a_cap, in_cap - a_cap)
            } else {
                panic!()
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

    fn shrink_capacity_merger_prio(
        &mut self,
        out_idx: EdgeIndex,
        prio_idx: EdgeIndex,
        other_idx: EdgeIndex,
    ) -> bool {
        let prio_cap = self[prio_idx].capacity;
        let other_cap = self[other_idx].capacity;
        let out_cap = self[out_idx].capacity;
        let in_cap = prio_cap + other_cap;

        let (new_out, new_prio, new_other) = if out_cap == in_cap {
            (out_cap, prio_cap, other_cap)
        } else if out_cap < in_cap {
            (in_cap, prio_cap, other_cap)
        } else if prio_cap >= in_cap {
            (in_cap, in_cap, 0.)
        } else {
            (in_cap, prio_cap, in_cap - prio_cap)
        };

        self[out_idx].capacity = new_out;
        self[prio_idx].capacity = new_prio;
        self[other_idx].capacity = new_other;

        (out_cap, prio_cap, other_cap) != (new_out, new_prio, new_other)
    }

    fn shrink_capacity_merger_no_prio(
        &mut self,
        out_idx: EdgeIndex,
        a_idx: EdgeIndex,
        b_idx: EdgeIndex,
    ) -> bool {
        false
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
        let mut graph = Compiler::new(entities).create_graph();
        graph.remove_false_io(&[4, 5, 6]);
        graph.simplify(&[]);
        graph.to_svg("3-2-broken.svg").unwrap();
    }

    #[test]
    fn belt_reduction() {
        let entities = load("tests/belt_reduction");
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[]);
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
        assert_eq!(graph.edge_weights().next().unwrap().capacity, 15.);
        graph.to_svg("belt_reduction.svg").unwrap();
    }

    #[test]
    fn splitter_reduction() {
        let entities = load("tests/splitter_reduction");
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[4]);
        assert_eq!(graph.node_count(), 4);
        assert_eq!(graph.edge_count(), 3);
        graph.to_svg("splitter_reduction.svg").unwrap();
    }

    #[test]
    fn splitter_merger_reduction() {
        let entities = load("tests/splitter_merger_reduction");
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[4, 5]);
        graph.to_svg("splitter_merger_reduction.svg").unwrap();
    }
}
