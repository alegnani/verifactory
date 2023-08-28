mod compile_entities;

use petgraph::Direction::{Incoming, Outgoing};
use relations::Relation;
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    hash::Hash,
    rc::Rc,
};

use crate::{
    entities::{BeltType, Entity, EntityId, Underground},
    ir::{Edge, FlowGraph, Input, Node, Output},
    utils::{Direction, Position},
};

use self::compile_entities::AddToGraph;

trait RelationMap<T>
where
    T: Eq + Hash + Clone + Debug,
{
    fn transpose(self) -> Self;
    fn to_relation(&self) -> Relation<T>;
    fn get_cardinality(&self, key: &T) -> i32;
    fn add(&mut self, key: &T, value: T) -> bool;
}

impl<T> RelationMap<T> for HashMap<T, HashSet<T>>
where
    T: Eq + Hash + Clone + Debug,
{
    fn transpose(self) -> Self {
        self.to_relation().transpose().to_hashmap()
    }

    fn to_relation(&self) -> Relation<T> {
        let iter_relation = self
            .iter()
            .flat_map(|(a, set)| set.iter().map(move |b| (a, b)));

        Relation::from_iter(iter_relation)
    }

    fn get_cardinality(&self, key: &T) -> i32 {
        self.get(key).map(|s| s.len()).unwrap_or(0) as i32
    }

    fn add(&mut self, key: &T, value: T) -> bool {
        match self.get_mut(key) {
            None => {
                let mut new = HashSet::new();
                new.insert(value);
                self.insert(key.clone(), new);
                true
            }
            Some(s) => s.insert(value),
        }
    }
}

type RelMap<T> = HashMap<T, HashSet<T>>;

/* XXX: do we really need the entities vector?
 * => remove Rc, get entities with pos_to_entity.values() */
pub struct Compiler {
    entities: Vec<Rc<Entity<i32>>>,
    positions: HashSet<Position<i32>>,
    belt_positions: HashSet<Position<i32>>,
    inserter_positions: HashSet<Position<i32>>,
    feeds_to: RelMap<Position<i32>>,
    feeds_from: RelMap<Position<i32>>,
    pos_to_entity: HashMap<Position<i32>, Rc<Entity<i32>>>,
}

impl Compiler {
    fn generate_pos_to_entity(
        entities: &Vec<Rc<Entity<i32>>>,
    ) -> HashMap<Position<i32>, Rc<Entity<i32>>> {
        let mut pos_to_entity = entities
            .iter()
            .map(|e| (e.get_base().position, e.clone()))
            .collect::<HashMap<_, _>>();

        for e in entities {
            if let Entity::Splitter(s) = **e {
                pos_to_entity.insert(s.get_phantom(), e.clone());
            }
        }
        pos_to_entity
    }

    fn generate_position_sets(
        pos_to_entity: &HashMap<Position<i32>, Rc<Entity<i32>>>,
    ) -> (
        HashSet<Position<i32>>,
        HashSet<Position<i32>>,
        HashSet<Position<i32>>,
    ) {
        let positions = pos_to_entity.keys().cloned().collect::<HashSet<_>>();

        let belt_positions = pos_to_entity
            .iter()
            .filter_map(|(k, v)| match **v {
                Entity::Belt(_) | Entity::Underground(_) | Entity::Splitter(_) => Some(*k),
                _ => None,
            })
            .collect();
        let inserter_positions = positions.difference(&belt_positions).cloned().collect();
        (positions, belt_positions, inserter_positions)
    }

    /// Creates a relation of positions that feed other positions
    ///
    /// Note that this can NOT be used to perform a reachability analysis as the two sides of the splitters are not connected.
    ///        __
    ///       |  \
    /// A ->  |   ⟩ -> C
    ///       |__/
    ///       |  \
    /// B ->  |   ⟩ -> D
    ///       |__/
    /// This only generates the following relation: {A->C, B->D}.
    /// To perform reachability analysis one would need to also include A->D and B->C.
    pub fn populate_feeds_to(
        pos_to_entity: &HashMap<Position<i32>, Rc<Entity<i32>>>,
        entities: &Vec<Rc<Entity<i32>>>,
    ) -> RelMap<Position<i32>> {
        let mut feeds_to = HashMap::new();

        fn add_feeds_to(
            feeds_to: &mut RelMap<Position<i32>>,
            pos_to_entity: &HashMap<Position<i32>, Rc<Entity<i32>>>,
            pos: Position<i32>,
            dir: Direction,
        ) {
            let dest = pos.shift(dir, 1);
            if let Some(e) = pos_to_entity.get(&dest) {
                match **e {
                    Entity::Belt(_) | Entity::Underground(_) | Entity::Splitter(_) => {
                        feeds_to.add(&pos, pos.shift(dir, 1));
                    }
                    _ => (),
                }
            }
        }

        let output_undergrounds = entities.iter().filter_map(|e| match **e {
            Entity::Underground(x) if x.belt_type == BeltType::Output => Some(e.clone()),
            _ => None,
        });

        for e in entities {
            let base = e.get_base();
            let dir = base.direction;
            let pos = base.position;
            match **e {
                Entity::Belt(_) => {
                    add_feeds_to(&mut feeds_to, pos_to_entity, pos, dir);
                }
                Entity::Underground(u) if u.belt_type == BeltType::Input => {
                    if let Some(output_pos) =
                        find_underground_output(&u, output_undergrounds.clone())
                    {
                        feeds_to.add(&pos, output_pos);
                    }
                }
                Entity::Underground(_) => {
                    add_feeds_to(&mut feeds_to, pos_to_entity, pos, dir);
                }
                Entity::Splitter(s) => {
                    add_feeds_to(&mut feeds_to, pos_to_entity, pos, dir);
                    let phantom = s.get_phantom();
                    add_feeds_to(&mut feeds_to, pos_to_entity, phantom, dir);
                }
                Entity::Inserter(l) => {
                    let source = l.get_source();
                    let destination = l.get_destination();
                    feeds_to.add(&source, destination);
                }
                Entity::LongInserter(l) => {
                    let source = l.get_source();
                    let destination = l.get_destination();
                    feeds_to.add(&source, destination);
                }
                Entity::Assembler(_) => (),
            };
        }
        /* validate that noting feeds into an output underground except for an input underground */
        for (source, set) in feeds_to.iter_mut() {
            set.retain(|dest| {
                let source_entity = pos_to_entity.get(source);
                let dest_entity = pos_to_entity.get(dest);
                if let (Some(source), Some(dest)) = (source_entity, dest_entity) {
                    let dest_is_output = matches!(**dest, Entity::Underground(x) if x.belt_type == BeltType::Output);
                    let source_is_input = matches!(**source, Entity::Underground(x) if x.belt_type == BeltType::Input);
                    return !dest_is_output || source_is_input;
                }
                true
            });
        }
        feeds_to.retain(|_, set| !set.is_empty());

        feeds_to
    }

    pub fn populate_feeds_from(
        pos_to_entity: &HashMap<Position<i32>, Rc<Entity<i32>>>,
        entities: &Vec<Rc<Entity<i32>>>,
    ) -> RelMap<Position<i32>> {
        Self::populate_feeds_to(pos_to_entity, entities).transpose()
    }
}

impl Compiler {
    pub fn new(entities: Vec<Entity<i32>>) -> Self {
        let entities: Vec<_> = entities.into_iter().map(Rc::new).collect();
        let pos_to_entity = Self::generate_pos_to_entity(&entities);

        let (positions, belt_positions, inserter_positions) =
            Self::generate_position_sets(&pos_to_entity);
        let feeds_to = Self::populate_feeds_to(&pos_to_entity, &entities);
        let feeds_from = Self::populate_feeds_from(&pos_to_entity, &entities);

        Self {
            entities,
            positions,
            belt_positions,
            inserter_positions,
            feeds_to,
            feeds_from,
            pos_to_entity,
        }
    }

    pub fn pos_to_id(&self, position: &Position<i32>) -> Option<EntityId> {
        self.pos_to_entity.get(position).map(|e| e.get_base().id)
    }

    /// Creates a relation of positions that feed other positions
    ///
    /// Usable to peform reachability analysis.
    /// ```
    ///        __
    ///       |  \
    /// A ->  |   ⟩ -> C
    ///       |__/
    ///       |  \
    /// B ->  |   ⟩ -> D
    ///       |__/
    /// ```
    /// Generates the following relation: {A->C, A->D, B->C, B->D}.
    pub fn feeds_to_reachability(&self) -> RelMap<Position<i32>> {
        let mut feeds_to = self.feeds_to.clone();

        for e in &self.entities {
            if let Entity::Splitter(s) = **e {
                let base = e.get_base();
                let pos = base.position;
                let dir = base.direction;

                let phantom = s.get_phantom();
                feeds_to.add(&phantom, pos.shift(dir, 1));
                feeds_to.add(&pos, phantom.shift(dir, 1));
            }
        }

        feeds_to
    }

    pub fn feeds_from_reachability(&self) -> RelMap<Position<i32>> {
        self.feeds_to_reachability().transpose()
    }
}

impl Compiler {
    pub fn find_input_positions(&self) -> Vec<Position<i32>> {
        self.belt_positions
            .iter()
            .filter(|k| self.feeds_from.get(k).is_none())
            .cloned()
            .collect()
    }

    pub fn find_output_positions(&self) -> Vec<Position<i32>> {
        self.belt_positions
            .iter()
            .filter(|k| self.feeds_to.get(k).is_none())
            .cloned()
            .collect()
    }

    pub fn create_graph(&self) -> FlowGraph {
        let mut graph = petgraph::Graph::new();

        let mut pos_to_connector = HashMap::new();

        for e in &self.entities {
            match **e {
                Entity::Splitter(splitter) => {
                    splitter.add_to_graph(&mut graph, &mut pos_to_connector)
                }
                Entity::Belt(belt) => belt.add_to_graph(&mut graph, &mut pos_to_connector),
                Entity::Underground(under) => under.add_to_graph(&mut graph, &mut pos_to_connector),
                _ => (),
            }
        }
        for (source, set) in &self.feeds_to {
            if let Some(source_idx) = pos_to_connector.get(source).map(|i| i.1) {
                for dest in set {
                    if let Some(dest_idx) = pos_to_connector.get(dest).map(|i| i.0) {
                        let edge = Edge {
                            side: None,
                            capacity: 69.0,
                        };
                        graph.add_edge(source_idx, dest_idx, edge);
                    }
                }
            }
        }
        /* promote suitable connectors to input or output nodes */
        for node in graph.node_indices() {
            if let Some(Node::Connector(c)) = graph.node_weight(node) {
                let id = c.id;
                let in_degree = graph.neighbors_directed(node, Incoming).count();
                let out_degree = graph.neighbors_directed(node, Outgoing).count();

                let is_output = out_degree == 0;
                let is_input = in_degree == 0;
                /* if the connector is not connected, leave it as is */
                if is_input ^ is_output {
                    let new_node = if is_input {
                        Node::Input(Input { id })
                    } else {
                        Node::Output(Output { id })
                    };
                    let node_ref = graph.node_weight_mut(node).unwrap();
                    *node_ref = new_node;
                }
            }
        }
        graph
    }
}

fn find_underground_output<I>(underground: &Underground<i32>, outputs: I) -> Option<Position<i32>>
where
    I: Iterator<Item = Rc<Entity<i32>>> + Clone,
{
    let base = underground.base;
    let pos = base.position;
    let dir = base.direction;
    let throughput = base.throughput;
    let max_distance = 3 + 2 * throughput as i32 / 15;
    /* online matching underground belt tiers can be connected */
    let outputs = outputs.filter(|u| u.get_base().throughput == throughput);
    /* XXX: runs in O(8n), with n = #outputs
     * can be improved to O(n) */
    for dist in 1..=max_distance {
        let possible_output_pos = pos.shift(dir, dist);
        for candidate in outputs.clone() {
            let candidate_base = candidate.get_base();
            if possible_output_pos == candidate_base.position {
                return Some(candidate_base.position);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use petgraph::dot::Dot;

    use crate::{
        import::string_to_entities,
        ir::{ShrinkStrength, Shrinkable},
    };

    use super::*;
    use std::fs;

    fn load(file: &str) -> Vec<Entity<i32>> {
        let blueprint_string = fs::read_to_string(file).unwrap();
        string_to_entities(&blueprint_string).unwrap()
    }

    #[test]
    fn feeds_to() {
        let entities = load("tests/feeds_from");
        let ctx = Compiler::new(entities);
        let feeds_to = ctx.feeds_to_reachability();
        let feeds_from = ctx.feeds_from_reachability();
        for (key, val) in &feeds_to {
            println!("{:?} --> {:?}", key, val)
        }
        assert_eq!(feeds_to, feeds_from.transpose());
    }

    #[test]
    fn inputs_generation() {
        let entities = load("test/input_output_gen");
        let ctx = Compiler::new(entities);
        let inputs = ctx.find_input_positions();
        println!("{:?}", inputs);
    }

    #[test]
    fn outputs_generation() {
        let entities = load("test/input_output_gen");
        let ctx = Compiler::new(entities);
        let outputs = ctx.find_output_positions();
        println!("{:?}", outputs);
    }

    #[test]
    fn compile_splitter() {
        let entities = load("test/input_output_gen");
        let ctx = Compiler::new(entities);
        let graph = ctx.create_graph();
        println!("{:?}", Dot::with_config(&graph, &[]));
    }

    #[test]
    fn graph_test() {
        let entities = load("tests/graph_test");
        let ctx = Compiler::new(entities);
        let graph = ctx.create_graph();
        println!("{:?}", Dot::with_config(&graph, &[]));
    }

    #[test]
    fn belt_weave() {
        let entities = load("tests/belt_weave");
        let ctx = Compiler::new(entities);
        let graph = ctx.create_graph();
        let graph = graph.shrink(ShrinkStrength::Aggressive);
        println!("{:?}", Dot::with_config(&graph, &[]));
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
    }
}
