use relations::Relation;
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    hash::Hash,
    rc::Rc,
};

use crate::{
    base_entity::EntityId,
    entities::{BeltType, Entity, EntityTrait, Underground},
    ir::{Input, Node},
    utils::{Direction, Position, Rotation},
};

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
                Entity::Inserter(_) => {
                    let input = pos.shift(dir.rotate(Rotation::Anticlockwise, 2), 1);
                    let output = pos.shift(dir, 1);
                    feeds_to.add(&input, output);
                }
                Entity::LongInserter(_) => {
                    let input = pos.shift(dir.rotate(Rotation::Anticlockwise, 2), 2);
                    let output = pos.shift(dir, 2);
                    feeds_to.add(&input, output);
                }
                Entity::Assembler(_) => (),
            };
        }
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

    pub fn generate_ir_inputs(&self) -> Vec<Node> {
        self.belt_positions
            .iter()
            .filter_map(|k| {
                if self.feeds_from.get(k).is_none() {
                    let id = self.pos_to_id(k).unwrap();
                    return Some(Node::Input(Input { id }));
                }
                None
            })
            .collect()
    }

    pub fn generate_ir_outputs(&self) -> Vec<Node> {
        self.belt_positions
            .iter()
            .filter_map(|k| {
                if self.feeds_to.get(k).is_none() {
                    let id = self.pos_to_id(k).unwrap();
                    return Some(Node::Input(Input { id }));
                }
                None
            })
            .collect()
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
    use crate::import::string_to_entities;

    use super::*;
    use std::fs;

    fn get_belt_entities() -> Vec<Entity<i32>> {
        let blueprint_string = fs::read_to_string("tests/feeds_from").unwrap();
        string_to_entities(&blueprint_string).unwrap()
    }

    fn get_io_test() -> Vec<Entity<i32>> {
        let blueprint_string = fs::read_to_string("tests/input_output_gen").unwrap();
        string_to_entities(&blueprint_string).unwrap()
    }

    #[test]
    fn feeds_to() {
        let entities = get_belt_entities();
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
        let entities = get_io_test();
        let ctx = Compiler::new(entities);
        let inputs = ctx.generate_ir_inputs();
        println!("{:?}", inputs);
    }

    #[test]
    fn outputs_generation() {
        let entities = get_io_test();
        let ctx = Compiler::new(entities);
        let outputs = ctx.generate_ir_outputs();
        println!("{:?}", outputs);
    }
}
