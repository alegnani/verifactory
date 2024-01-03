use fraction::GenericFraction;

use crate::{entities::EntityId, utils::Side};
use petgraph::prelude::{EdgeIndex, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction::{Incoming, Outgoing};
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub enum Node {
    /// See [`Splitter`]
    ///
    /// Element with in_deg = 1 and out_deg = 2.
    Splitter(Splitter),
    /// See [`Merger`]
    ///
    /// Element with in_deg = 2 and out_deg = 1
    Merger(Merger),
    /// See [`Connector`]
    ///
    /// Element with in_deg = 1 and out_deg = 1
    Connector(Connector),
    /// See [`Input`]
    ///
    /// Element with in_deg = 0 and out_deg = 1
    Input(Input),
    /// See [`Output`]
    ///
    /// Element with in_deg = 1 and out_deg = 0
    Output(Output),
}

impl Node {
    pub fn get_id(&self) -> EntityId {
        match self {
            Node::Connector(c) => c.id,
            Node::Input(i) => i.id,
            Node::Merger(m) => m.id,
            Node::Output(o) => o.id,
            Node::Splitter(s) => s.id,
        }
    }

    pub fn get_str(&self) -> String {
        let prefix = match self {
            Node::Connector(_) => "c",
            Node::Input(_) => "i",
            Node::Merger(_) => "m",
            Node::Output(_) => "o",
            Node::Splitter(_) => "s",
        };
        format!("{}{}", prefix, self.get_id())
    }
}

/// Element that merges two inputs into a single output, optionally prioritizing one side.
#[derive(Debug, Clone)]
pub struct Merger {
    pub input_priority: Side,
    /// What entity this corresponds to
    pub id: EntityId,
}

/// A components that represents a single belt.
/// It's additionally used to model the input and output of splitters and mergers.
///
/// Each connector *must* have in_degree and out_degree equal to 1.
///
/// Each path of connectors `A-C-C-...-C-B`, where `C` is a connector and `A,B` are not, can be
/// transformed to `A-B`.
#[derive(Debug, Clone)]
pub struct Connector {
    /// What entity this connector corresponds to
    pub id: EntityId,
}

/// A node that has no ingoing edges
#[derive(Debug, Clone)]
pub struct Input {
    /// What entity this connector corresponds to
    pub id: EntityId,
}

/// A node that has no outgoing edges
#[derive(Debug, Clone)]
pub struct Output {
    /// What entity this connector corresponds to
    pub id: EntityId,
}

/// Element that splits a single input into two outputs, optionally prioritizing one side.
#[derive(Debug, Clone)]
pub struct Splitter {
    pub output_priority: Side,
    /// What entity this corresponds to
    pub id: EntityId,
}

pub trait Lattice {
    /// Compute the meet operation of two elements of a lattice
    ///
    /// Logically similar to an `AND`.
    fn meet(&self, other: &Self) -> Self;
    /// Compute the join operation of two elements of a lattice
    ///
    /// Logically similar to an `OR`.
    fn join(&self, other: &Self) -> Self;
    /// Returns `true` if a join without ambiguity is possible
    fn can_join(&self, _other: &Self) -> bool {
        true
    }
}

impl Lattice for Side {
    fn meet(&self, other: &Self) -> Self {
        match (self, other) {
            (x, y) if x == y => *x,
            _ => Self::None,
        }
    }

    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::None, x) | (x, Self::None) => *x,
            (x, y) if x == y => *x,
            _ => panic!(),
        }
    }

    fn can_join(&self, other: &Self) -> bool {
        !matches!(
            (self, other),
            (Self::Left, Self::Right) | (Self::Right, Self::Left)
        )
    }
}

/// An edge connecting two nodes
#[derive(Clone, Copy)]
pub struct Edge {
    /// The side this edge corresponds to, if applicable. E.g. a belt's left or right side.
    pub side: Side,
    /// Capacity in items/s
    ///
    /// For example, if this represents a line of belts, the capacity is the min capacity
    /// of all belts in the line.
    pub capacity: GenericFraction<u128>,
}

impl Debug for Edge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let denom = *self.capacity.denom().unwrap() as f64;
        let numer = *self.capacity.numer().unwrap() as f64;
        f.debug_struct("Edge")
            .field("side", &self.side)
            .field("capacity", &(numer / denom))
            .finish()
    }
}

impl Lattice for Edge {
    fn meet(&self, other: &Self) -> Self {
        let side = self.side.meet(&other.side);
        let capacity = self.capacity.min(other.capacity);
        Self { side, capacity }
    }

    fn join(&self, other: &Self) -> Self {
        let side = self.side.join(&other.side);
        /* should be max but we don't want this kind of join */
        let capacity = self.capacity.min(other.capacity);
        Self { side, capacity }
    }

    fn can_join(&self, other: &Self) -> bool {
        self.side.can_join(&other.side)
    }
}

/// Graph of the IR
pub type FlowGraph = petgraph::Graph<Node, Edge, petgraph::Directed>;

pub trait GraphHelper {
    /// Returns the in-degree of the given node at `node_idx`
    fn in_deg(&self, node_idx: NodeIndex) -> usize;
    /// Returns the out-degree of the given node at `node_idx`
    fn out_deg(&self, node_idx: NodeIndex) -> usize;

    /// Returns the neighbouring nodes of the given node at `node_idx`, having an edge going to the `node_idx`.
    fn in_nodes(&self, node_idx: NodeIndex) -> Vec<NodeIndex>;
    /// Returns the neighbouring nodes of the given node at `node_idx`, having an edge going from the `node_idx`.
    fn out_nodes(&self, node_idx: NodeIndex) -> Vec<NodeIndex>;

    /// Returns shared references to the inbound edges of the node at `node_idx`
    fn in_edges(&self, node_idx: NodeIndex) -> Vec<&Edge>;
    /// Returns shared references to the outbound edges of the node at `node_idx`
    fn out_edges(&self, node_idx: NodeIndex) -> Vec<&Edge>;

    /// Returns the `EdgeIndex` to the inbound edges of the node at `node_idx`
    fn in_edge_idx(&self, node_idx: NodeIndex) -> Vec<EdgeIndex>;
    /// Returns the `EdgeIndex` to the outbound edges of the node at `node_idx`
    fn out_edge_idx(&self, node_idx: NodeIndex) -> Vec<EdgeIndex>;

    /// Returns the `EdgeIndex` of the edge from/to `node_idx`, going in the given direction and having the correct `Side` label.
    ///
    /// # Panics
    ///
    /// Panics if there is no edge matching all the constraints.
    fn get_edge(&self, node_idx: NodeIndex, dir: petgraph::Direction, side: Side) -> EdgeIndex;
}

impl GraphHelper for FlowGraph {
    fn in_deg(&self, node_idx: NodeIndex) -> usize {
        self.edges_directed(node_idx, Incoming).count()
    }

    fn out_deg(&self, node_idx: NodeIndex) -> usize {
        self.edges_directed(node_idx, Outgoing).count()
    }

    fn in_nodes(&self, node_idx: NodeIndex) -> Vec<NodeIndex> {
        self.edges_directed(node_idx, Incoming)
            .map(|e| e.source())
            .collect()
    }

    fn out_nodes(&self, node_idx: NodeIndex) -> Vec<NodeIndex> {
        self.edges_directed(node_idx, Outgoing)
            .map(|e| e.target())
            .collect()
    }

    fn in_edges(&self, node_idx: NodeIndex) -> Vec<&Edge> {
        self.edges_directed(node_idx, Incoming)
            .map(|e| e.weight())
            .collect()
    }

    fn out_edges(&self, node_idx: NodeIndex) -> Vec<&Edge> {
        self.edges_directed(node_idx, Outgoing)
            .map(|e| e.weight())
            .collect()
    }

    fn out_edge_idx(&self, node_idx: NodeIndex) -> Vec<EdgeIndex> {
        self.edges_directed(node_idx, Outgoing)
            .map(|e| e.id())
            .collect()
    }

    fn in_edge_idx(&self, node_idx: NodeIndex) -> Vec<EdgeIndex> {
        self.edges_directed(node_idx, Incoming)
            .map(|e| e.id())
            .collect()
    }

    fn get_edge(&self, node_idx: NodeIndex, dir: petgraph::Direction, side: Side) -> EdgeIndex {
        self.edges_directed(node_idx, dir)
            .find(|e| e.weight().side == side)
            .map(|e| e.id())
            .unwrap()
    }
}
