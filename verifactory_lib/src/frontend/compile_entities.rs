use fraction::GenericFraction;
use petgraph::prelude::NodeIndex;
use std::collections::HashMap;

use crate::{
    entities::{FBBelt, FBEntity, FBSplitter, FBUnderground},
    ir::{self, Connector, Edge, FlowGraph, Node},
    utils::{Position, Side},
};

fn add_belt_to_graph(
    belt: &FBEntity<i32>,
    graph: &mut FlowGraph,
    pos_to_connector: &mut HashMap<Position<i32>, (NodeIndex, NodeIndex)>,
) {
    let base = belt.get_base();
    let id = base.id;
    let capacity = base.throughput.into();

    /* add the nodes to the graph */
    let input = Node::Connector(Connector { id });
    let output = Node::Connector(Connector { id });
    let in_idx = graph.add_node(input);
    let out_idx = graph.add_node(output);

    /* add the nodes to the connector map */
    let pos = base.position;
    pos_to_connector.insert(pos, (in_idx, out_idx));

    /* add the edges */
    let edge = Edge {
        side: Side::None,
        capacity,
    };

    graph.add_edge(in_idx, out_idx, edge);
}

pub trait AddToGraph {
    fn add_to_graph(
        &self,
        graph: &mut FlowGraph,
        pos_to_connector: &mut HashMap<Position<i32>, (NodeIndex, NodeIndex)>,
    );
}

impl AddToGraph for FBBelt<i32> {
    fn add_to_graph(
        &self,
        graph: &mut FlowGraph,
        pos_to_connector: &mut HashMap<Position<i32>, (NodeIndex, NodeIndex)>,
    ) {
        add_belt_to_graph(&FBEntity::Belt(*self), graph, pos_to_connector)
    }
}

impl AddToGraph for FBUnderground<i32> {
    fn add_to_graph(
        &self,
        graph: &mut FlowGraph,
        pos_to_connector: &mut HashMap<Position<i32>, (NodeIndex, NodeIndex)>,
    ) {
        add_belt_to_graph(&FBEntity::Underground(*self), graph, pos_to_connector)
    }
}

impl AddToGraph for FBSplitter<i32> {
    fn add_to_graph(
        &self,
        graph: &mut FlowGraph,
        pos_to_connector: &mut HashMap<Position<i32>, (NodeIndex, NodeIndex)>,
    ) {
        let id = self.base.id;

        let ir_merger = ir::Merger {
            input_priority: self.input_prio.into(),
            id,
        };
        let ir_splitter = ir::Splitter {
            output_priority: self.output_prio.into(),
            id,
        };
        let capacity = self.base.throughput.into();

        /* add the nodes to the graph */
        let splitter_idx = graph.add_node(Node::Splitter(ir_splitter));
        let merger_idx = graph.add_node(Node::Merger(ir_merger));

        let in_r = Node::Connector(Connector { id });
        let out_r = Node::Connector(Connector { id });
        let in_r_idx = graph.add_node(in_r);
        let out_r_idx = graph.add_node(out_r);

        let in_l = Node::Connector(Connector { id });
        let out_l = Node::Connector(Connector { id });
        let in_l_idx = graph.add_node(in_l);
        let out_l_idx = graph.add_node(out_l);

        /* add the nodes to the connector map */
        let pos_r = self.base.position;
        let pos_l = self.get_phantom().base.position;
        pos_to_connector.insert(pos_r, (in_r_idx, out_r_idx));
        pos_to_connector.insert(pos_l, (in_l_idx, out_l_idx));

        /* add the edges */
        let merger_splitter_edge = Edge {
            side: Side::None,
            capacity: capacity * GenericFraction::new(2u128, 1u128),
        };
        let r_edge = Edge {
            side: Side::Right,
            capacity,
        };
        let l_edge = Edge {
            side: Side::Left,
            capacity,
        };

        graph.add_edge(in_l_idx, merger_idx, l_edge);
        graph.add_edge(in_r_idx, merger_idx, r_edge);

        graph.add_edge(splitter_idx, out_l_idx, l_edge);
        graph.add_edge(splitter_idx, out_r_idx, r_edge);

        graph.add_edge(merger_idx, splitter_idx, merger_splitter_edge);
    }
}
