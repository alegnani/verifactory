//! The intermediate representaton used for the conversion between a factorio blue

mod graph_algos;

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

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Side {
    Left,
    Right,
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

/// An edge connecting two nodes
#[derive(Debug, Clone, Copy)]
pub struct Edge {
    /// The side this edge corresponds to, if applicable. E.g. a belt's left or right side.
    pub side: Option<Side>,
    /// Capacity in items/s
    ///
    /// For example, if this represents a line of belts, the capacity is the min capacity
    /// of all belts in the line.
    pub capacity: f64,
}

pub type FlowGraph = petgraph::Graph<Node, Edge, petgraph::Directed>;
