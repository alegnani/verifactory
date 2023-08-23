use anyhow::{anyhow, Context, Result};
use base64::engine::{general_purpose, Engine as _};
use inflate::inflate_bytes_zlib;
use serde_json::Value;

use crate::{
    base_entity::BaseEntity,
    entities::{Belt, Entity, EntityTrait, Splitter, Underground},
    utils::{
        Direction::{East, West},
        Position,
        Rotation::Anticlockwise,
    },
};

fn decompress_string(blueprint_string: &str) -> Result<Value> {
    let skip_first_byte = &blueprint_string.as_bytes()[1..blueprint_string.len()];
    let base64_decoded = general_purpose::STANDARD.decode(skip_first_byte)?;
    let decoded = inflate_bytes_zlib(&base64_decoded).map_err(|s| anyhow!(s))?;
    Ok(serde_json::from_slice(&decoded)?)
}

fn get_json_entities(json: Value) -> Result<Vec<Value>> {
    json.get("blueprint")
        .context("No blueprint key in json")?
        .get("entities")
        .context("No entities key in blueprint")?
        .as_array()
        .context("Entities are not an array")
        .map(|v| v.to_owned())
}

fn snap_to_grid(entities: &[Entity<f64>]) {
    /* snap splitters to the grid as they are offset by 0.5 */
    for e in entities {
        if let Entity::Splitter(mut splitter) = e {
            let shift_dir = splitter.get_base().direction.rotate(Anticlockwise, 1);
            /* in Factorio blueprints the y-axis is inverted */
            let shift_dir = match shift_dir {
                East => West,
                West => East,
                x => x,
            };
            splitter.get_base_mut().shift_mut(shift_dir, 0.5);
        }
    }
}

fn normalize_entities(entities: &[Entity<f64>]) -> Vec<Entity<i32>> {
    let max_y = entities
        .iter()
        .map(|e| e.get_base().position.y)
        .fold(f64::NAN, f64::max);

    let min_x = entities
        .iter()
        .map(|e| e.get_base().position.x)
        .fold(f64::NAN, f64::min);

    entities
        .iter()
        .map(|e| {
            let base = e.get_base();
            let x = (base.position.x - min_x) as i32;
            /* uninvert the y-axis */
            let y = (max_y - base.position.y) as i32;
            let position = Position { x, y };
            let base = BaseEntity {
                position,
                id: base.id,
                direction: base.direction,
                throughput: base.throughput,
            };
            match e {
                Entity::Belt(_) => Entity::Belt(Belt { base }),
                Entity::Underground(u) => Entity::Underground(Underground {
                    base,
                    belt_type: u.belt_type,
                }),
                Entity::Splitter(s) => Entity::Splitter(Splitter {
                    base,
                    input_prio: s.input_prio,
                    output_prio: s.output_prio,
                }),
            }
        })
        .collect()
}

pub fn string_to_entities(blueprint_string: &str) -> Result<Vec<Entity<i32>>> {
    let json = decompress_string(blueprint_string)?;
    let entities: Vec<_> = get_json_entities(json)?
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<Vec<_>, _>>()?;

    snap_to_grid(&entities);
    Ok(normalize_entities(&entities))
}

#[cfg(test)]
mod tests {
    use crate::{
        entities::{BeltType, Priority},
        utils::Direction,
    };

    use super::*;
    use std::fs;
    fn get_entities() -> Vec<Entity<i32>> {
        let blueprint_string = fs::read_to_string("tests/test_blueprint").unwrap();
        string_to_entities(&blueprint_string).unwrap()
    }

    #[test]
    fn throughput_tiers() {
        let entities = get_entities();

        let mut throughput = [0, 0, 0];
        for e in entities {
            let index = (e.get_base().throughput / 15.0 - 1.0) as usize;
            throughput[index] += 1;
        }
        assert_eq!(throughput, [3, 4, 1]);
    }

    #[test]
    fn belt_direction() {
        let entities = get_entities();
        for e in entities {
            if let Entity::Belt(b) = e {
                let throughput = b.base.throughput;
                match b.base.direction {
                    Direction::North => assert_eq!(throughput, 30.0),
                    Direction::South => assert_eq!(throughput, 45.0),
                    _ => assert_eq!(throughput, 15.0),
                }
            }
        }
    }

    #[test]
    fn splitter_prio() {
        let entities = get_entities();
        for e in entities {
            if let Entity::Splitter(s) = e {
                if s.input_prio == Priority::None {
                    assert_eq!(s.output_prio, Priority::Left);
                } else {
                    assert_eq!(s.input_prio, Priority::Left);
                    assert_eq!(s.output_prio, Priority::Right);
                }
            }
        }
    }

    #[test]
    fn underground_type() {
        let entities = get_entities();
        for e in entities {
            if let Entity::Underground(u) = e {
                if u.base.position.y == 0 {
                    assert_eq!(u.belt_type, BeltType::Input);
                } else {
                    assert_eq!(u.belt_type, BeltType::Output);
                }
            }
        }
    }
}
