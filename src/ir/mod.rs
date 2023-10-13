//! The intermediate representaton used for the conversion between a factorio blue

mod graph_algos;
mod reverse;

pub use self::reverse::Reversable;
use std::fmt::Debug;

use fraction::GenericFraction;
pub use graph_algos::*;

use petgraph::{
    prelude::{EdgeIndex, NodeIndex},
    visit::EdgeRef,
    Direction::{Incoming, Outgoing},
};

use crate::entities::{EntityId, Priority};

/// An entity in the intermerdiate representation can either be a splitter or a merger.
///
/// A splitter takes a single input and splits it in two, and optionally prioritizes
/// an output over the other.
///
/// A merger takes two inputs, optionally prioritizing one, and combines them into
/// a single output.
///
/// A belt is represented by two different sides.
///
/// # Examples
///
/// ## Belt side junction
///
/// ```
/// ⇉⇉⇉⇉⇉⇉
///    ⇈
///    ⇈
///    ⇈
///    
/// ```
///
/// A vertical belt joins an horizontal one from the side. The two sides
/// of the vertical one are merged, with priority given to the left one.
/// Then this combination is merged with the right side of the horizontal one, with
/// priority given to the horizontal belt.
///
///
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
}

/// Element that merges two inputs into a single output, optionally prioritizing one side.
#[derive(Debug, Clone)]
pub struct Merger {
    pub input_priority: Option<Side>,
    /// What entity this corresponds to
    pub id: EntityId,
}

/// A components that only exists for debugging purposes. A path of connectors can represent,
/// for example, a line of belts with no mergers/splitters/...
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
    pub output_priority: Option<Side>,
    /// What entity this corresponds to
    pub id: EntityId,
}

pub trait Lattice {
    fn meet(&self, other: &Self) -> Self;
    fn join(&self, other: &Self) -> Self;
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn other(&self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

impl From<Priority> for Option<Side> {
    fn from(value: Priority) -> Self {
        match value {
            Priority::None => None,
            Priority::Left => Some(Side::Left),
            Priority::Right => Some(Side::Right),
        }
    }
}

impl Lattice for Option<Side> {
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (None, x) | (x, None) => *x,
            (Some(x), Some(y)) if x == y => Some(*x),
            _ => panic!(),
        }
    }
    fn meet(&self, other: &Self) -> Self {
        match (self, other) {
            (Some(x), Some(y)) if y == x => Some(*x),
            _ => None,
        }
    }
}

/// An edge connecting two nodes
#[derive(Clone, Copy)]
pub struct Edge {
    /// The side this edge corresponds to, if applicable. E.g. a belt's left or right side.
    pub side: Option<Side>,
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
}

pub type FlowGraph = petgraph::Graph<Node, Edge, petgraph::Directed>;

pub trait GraphHelper {
    fn in_deg(&self, node_idx: NodeIndex) -> usize;
    fn out_deg(&self, node_idx: NodeIndex) -> usize;

    fn in_nodes(&self, node_idx: NodeIndex) -> Vec<NodeIndex>;
    fn out_nodes(&self, node_idx: NodeIndex) -> Vec<NodeIndex>;

    fn in_edges(&self, node_idx: NodeIndex) -> Vec<&Edge>;
    fn out_edges(&self, node_idx: NodeIndex) -> Vec<&Edge>;

    fn in_edge_idx(&self, node_idx: NodeIndex) -> Vec<EdgeIndex>;
    fn out_edge_idx(&self, node_idx: NodeIndex) -> Vec<EdgeIndex>;

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
            .find(|e| matches!(e.weight().side, Some(x) if x == side))
            .map(|e| e.id())
            .unwrap()
    }
}
