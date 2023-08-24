use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};

use crate::{
    entities::{BeltType, Entity, EntityTrait, Underground},
    utils::{Position, Rotation},
};

#[derive(Debug)]
pub struct FeedsMap {
    map: HashMap<Position<i32>, HashSet<Position<i32>>>,
}

impl FeedsMap {
    pub fn new() -> Self {
        let map = HashMap::new();
        Self { map }
    }

    pub fn add(&mut self, key: &Position<i32>, value: Position<i32>) -> bool {
        let set = self.map.get_mut(key);
        match set {
            None => {
                let mut new = HashSet::new();
                new.insert(value);
                self.map.insert(*key, new);
                true
            }
            Some(s) => s.insert(value),
        }
    }

    pub fn get_set(&self, key: &Position<i32>) -> Result<&HashSet<Position<i32>>> {
        self.map.get(key).context("No entity feeds this position")
    }

    pub fn get_set_card(&self, key: &Position<i32>) -> i32 {
        self.get_set(key).map(|s| s.len()).unwrap_or(0) as i32
    }
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
pub fn populate_feeds_from(entities: &[Entity<i32>]) -> FeedsMap {
    let mut feeds_from = FeedsMap::new();

    let output_undergrounds = entities
        .iter()
        .filter(|&e| matches!(e, Entity::Underground(x) if x.belt_type == BeltType::Output));

    for e in entities {
        let base = e.get_base();
        let dir = base.direction;
        let pos = base.position;
        match e {
            Entity::Belt(_) => {
                feeds_from.add(&pos, pos.shift(dir, 1));
            }
            Entity::Underground(u) if u.belt_type == BeltType::Input => {
                if let Some(output_pos) = find_underground_output(u, output_undergrounds.clone()) {
                    feeds_from.add(&pos, output_pos);
                }
            }
            Entity::Underground(_) => {
                feeds_from.add(&pos, pos.shift(dir, 1));
            }
            Entity::Splitter(_) => {
                feeds_from.add(&pos, pos.shift(dir, 1));
                let phantom = pos.shift(dir.rotate(Rotation::Anticlockwise, 1), 1);
                feeds_from.add(&phantom, phantom.shift(dir, 1));
            }
            Entity::Inserter(_) => {
                let input = pos.shift(dir.rotate(Rotation::Anticlockwise, 2), 1);
                let output = pos.shift(dir, 1);
                feeds_from.add(&input, output);
            }
            Entity::LongInserter(_) => {
                let input = pos.shift(dir.rotate(Rotation::Anticlockwise, 2), 2);
                let output = pos.shift(dir, 2);
                feeds_from.add(&input, output);
            }
            Entity::Assembler(_) => (),
        };
    }
    feeds_from
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
pub fn populate_feeds_from_reachability(entities: &[Entity<i32>]) -> FeedsMap {
    let mut feeds_from = populate_feeds_from(entities);

    for e in entities {
        if let Entity::Splitter(_) = e {
            let base = e.get_base();
            let pos = base.position;
            let dir = base.direction;

            let phantom = pos.shift(dir.rotate(Rotation::Anticlockwise, 1), 1);
            feeds_from.add(&phantom, pos.shift(dir, 1));
            feeds_from.add(&pos, phantom.shift(dir, 1));
        }
    }

    feeds_from
}

fn find_underground_output<'a, I>(
    underground: &Underground<i32>,
    outputs: I,
) -> Option<Position<i32>>
where
    I: Iterator<Item = &'a Entity<i32>> + Clone,
{
    let base = underground.base;
    let pos = base.position;
    let dir = base.direction;
    let throughput = base.throughput;
    let max_distance = 3 + 2 * throughput as i32 / 15;
    /* online matching underground belt tiers can be connected */
    let outputs = outputs.filter(|&u| u.get_base().throughput == throughput);
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

    #[test]
    fn feeds_from() {
        let entities = get_belt_entities();
        let feeds_from = populate_feeds_from(&entities);
        for (key, val) in feeds_from.map {
            println!("{:?} --> {:?}", key, val)
        }
    }
}
