//! Utility functions to load Factorio blueprint strings

use anyhow::{anyhow, Context, Result};
use base64::engine::{general_purpose, Engine as _};
use inflate::inflate_bytes_zlib;
use serde::{de::Error, Deserialize, Deserializer};
use serde_json::Value;

use crate::{
    entities::{
        Assembler, BaseEntity, Belt, Entity, Inserter, LongInserter, Priority, Splitter,
        SplitterPhantom, Underground,
    },
    utils::{
        Direction::{self, East, West},
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

impl<'de> Deserialize<'de> for BaseEntity<f64> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value: Value = Deserialize::deserialize(deserializer)?;

        let id = value
            .get("entity_number")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .ok_or(Error::missing_field("entity_number"))?;

        let position: Position<f64> = value
            .get("position")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .ok_or(Error::missing_field("position"))?;

        let direction = value
            .get("direction")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or(Direction::North);

        let base = BaseEntity {
            id,
            position,
            direction,
            throughput: 0.0,
        };
        Ok(base)
    }
}

impl<'de> Deserialize<'de> for Entity<f64> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value: Value = Deserialize::deserialize(deserializer)?;

        let name = value
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or(Error::missing_field("name"))?;

        let mut base: BaseEntity<f64> = serde_json::from_value(value.clone())
            .map_err(|_| Error::custom("Could not deserialize BaseEntity"))?;
        base.throughput = if name.contains("express") {
            45.0
        } else if name.contains("fast") {
            30.0
        } else {
            15.0
        };

        if name.contains("transport-belt") {
            Ok(Self::Belt(Belt { base }))
        } else if name.contains("underground-belt") {
            let belt_type = value
                .get("type")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .ok_or(Error::missing_field("type"))?;

            Ok(Self::Underground(Underground { base, belt_type }))
        } else if name.contains("splitter") {
            let input_prio = value
                .get("input_priority")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or(Priority::None);

            let output_prio = value
                .get("output_priority")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or(Priority::None);

            Ok(Self::Splitter(Splitter {
                base,
                input_prio,
                output_prio,
            }))
        } else if name.contains("inserter") {
            if name.contains("long-handed") {
                base.throughput = 1.2;
                return Ok(Self::LongInserter(LongInserter { base }));
            }
            base.throughput = if name == "inserter" {
                0.83
            } else if name.contains("burner") {
                0.6
            } else {
                2.31
            };
            Ok(Self::Inserter(Inserter { base }))
        } else if name.contains("assembling-machine") {
            let tier = name
                .strip_prefix("assembling-machine-")
                .ok_or(Error::custom(
                    "Error whilst deserializing assembling machine tier",
                ))?;
            base.throughput = match tier {
                "1" => 0.5,
                "2" => 0.75,
                "3" => 1.25,
                _ => panic!(),
            };
            Ok(Self::Assembler(Assembler { base }))
        } else {
            Err(format!("Invalid entity: ({})", name)).map_err(serde::de::Error::custom)
        }
    }
}

fn snap_to_grid(entities: &mut [Entity<f64>]) {
    for e in entities {
        match e {
            /* snap splitters to the grid as they are offset by 0.5 */
            Entity::Splitter(splitter) => {
                let shift_dir = splitter.base.direction.rotate(Anticlockwise, 1);
                /* in Factorio blueprints the y-axis is inverted */
                let shift_dir = match shift_dir {
                    East => West,
                    West => East,
                    x => x,
                };
                splitter.base.shift_mut(shift_dir, 0.5);
            }
            /* flip direction of inserters */
            Entity::Inserter(inserter) => {
                let dir = inserter.base.direction;
                inserter.base.direction = dir.flip();
            }
            /* flip direction of long inserters */
            Entity::LongInserter(inserter) => {
                let dir = inserter.base.direction;
                inserter.base.direction = dir.flip();
            }
            _ => (),
        }
    }
}

fn normalize_entities(entities: &[Entity<f64>]) -> Vec<Entity<i32>> {
    let padding = 2.0;
    let max_y = entities
        .iter()
        .map(|e| e.get_base().position.y)
        .fold(f64::NAN, f64::max)
        + padding;

    let min_x = entities
        .iter()
        .map(|e| e.get_base().position.x)
        .fold(f64::NAN, f64::min)
        - padding;

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
                Entity::SplitterPhantom(_) => Entity::SplitterPhantom(SplitterPhantom { base }),
                Entity::Inserter(_) => Entity::Inserter(Inserter { base }),
                Entity::LongInserter(_) => Entity::LongInserter(LongInserter { base }),
                Entity::Assembler(_) => Entity::Assembler(Assembler { base }),
            }
        })
        .collect()
}

pub fn string_to_entities(blueprint_string: &str) -> Result<Vec<Entity<i32>>> {
    let json = decompress_string(blueprint_string)?;
    let mut entities: Vec<_> = get_json_entities(json)?
        .into_iter()
        .flat_map(serde_json::from_value)
        .collect::<Vec<_>>();

    snap_to_grid(&mut entities);
    let mut entities = normalize_entities(&entities);

    /* add splitter phantoms */
    let phantoms = entities
        .iter()
        .filter_map(|&e| match e {
            Entity::Splitter(s) => Some(Entity::SplitterPhantom(s.get_phantom())),
            _ => None,
        })
        .collect::<Vec<_>>();
    entities.extend(phantoms);
    Ok(entities)
}

#[cfg(test)]
mod tests {
    use crate::{
        entities::{BeltType, Priority},
        utils::Direction,
    };

    use super::*;
    use std::fs;
    fn get_belt_entities() -> Vec<Entity<i32>> {
        let blueprint_string = fs::read_to_string("tests/belts").unwrap();
        string_to_entities(&blueprint_string).unwrap()
    }

    fn get_assembly_entities() -> Vec<Entity<i32>> {
        let blueprint_string = fs::read_to_string("tests/inserter_assembler").unwrap();
        string_to_entities(&blueprint_string).unwrap()
    }

    #[test]
    fn throughput_tiers() {
        let entities = get_belt_entities();

        let mut throughput = [0, 0, 0];
        for e in entities {
            let index = (e.get_base().throughput / 15.0 - 1.0) as usize;
            throughput[index] += 1;
        }
        assert_eq!(throughput, [3, 4, 1]);
    }

    #[test]
    fn belt_direction() {
        let entities = get_belt_entities();
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
        let entities = get_belt_entities();
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
        let entities = get_belt_entities();
        for e in entities {
            if let Entity::Underground(u) = e {
                if u.base.position.y == 2 {
                    assert_eq!(u.belt_type, BeltType::Input);
                } else {
                    assert_eq!(u.belt_type, BeltType::Output);
                }
            }
        }
    }

    #[test]
    fn inserters_tier() {
        let entities = get_assembly_entities();
        for e in entities {
            if let Entity::Inserter(i) = e {
                let throughput = i.base.throughput;
                match i.base.direction {
                    Direction::North => assert_eq!(throughput, 2.31),
                    Direction::East => assert_eq!(throughput, 0.83),
                    _ => panic!(),
                }
            }
        }
    }

    #[test]
    fn long_inserter() {
        let entities = get_assembly_entities();
        for e in entities {
            if let Entity::LongInserter(l) = e {
                assert_eq!(l.base.direction, Direction::South);
                assert_eq!(l.base.throughput, 1.2);
            }
        }
    }

    #[test]
    fn assembler() {
        let entities = get_assembly_entities();
        for e in entities {
            if let Entity::Assembler(a) = e {
                assert_eq!(a.base.throughput, 1.25);
            }
        }
    }
}
