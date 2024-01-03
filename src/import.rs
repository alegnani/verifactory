//! Utility functions to convert a Factorio blueprint string into a list of `FBEntity`s.
//! A description of the JSON representation of the blueprint string can be found [here](https://wiki.factorio.com/Blueprint_string_format).

use anyhow::{anyhow, Context, Result};
use base64::engine::{general_purpose, Engine as _};
use inflate::inflate_bytes_zlib;
use serde::{de::Error, Deserialize, Deserializer};
use serde_json::Value;
use std::fs;

use crate::{
    entities::*,
    utils::{Direction, Position, Rotation},
};

/// Decompresses the string such that it can be interpreted as a JSON.
fn decompress_string(blueprint_string: &str) -> Result<Value> {
    let skip_first_byte = &blueprint_string.as_bytes()[1..blueprint_string.len()];
    let base64_decoded = general_purpose::STANDARD.decode(skip_first_byte)?;
    let decoded = inflate_bytes_zlib(&base64_decoded).map_err(|s| anyhow!(s))?;
    Ok(serde_json::from_slice(&decoded)?)
}

/// Turns a JSON string into a list of JSON substrings, each representing an entity of the blueprint.
fn get_json_entities(json: Value) -> Result<Vec<Value>> {
    json.get("blueprint")
        .context("No blueprint key in json")?
        .get("entities")
        .context("No entities key in blueprint")?
        .as_array()
        .context("Entities are not an array")
        .map(|v| v.to_owned())
}

/// Helper function that deserializes the attributes shared by each entity.
impl<'de> Deserialize<'de> for FBBaseEntity<f64> {
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

        let base = FBBaseEntity {
            id,
            position,
            direction,
            throughput: 0.0,
        };
        Ok(base)
    }
}

/// Deserialization function turning each JSON string into a `FBEntity<f64>`.
impl<'de> Deserialize<'de> for FBEntity<f64> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value: Value = Deserialize::deserialize(deserializer)?;

        let name = value
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or(Error::missing_field("name"))?;

        let mut base: FBBaseEntity<f64> = serde_json::from_value(value.clone())
            .map_err(|_| Error::custom("Could not deserialize BaseEntity"))?;
        base.throughput = if name.contains("express") {
            45.0
        } else if name.contains("fast") {
            30.0
        } else {
            15.0
        };

        if name.contains("transport-belt") {
            Ok(Self::Belt(FBBelt { base }))
        } else if name.contains("underground-belt") {
            let belt_type = value
                .get("type")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .ok_or(Error::missing_field("type"))?;

            Ok(Self::Underground(FBUnderground { base, belt_type }))
        } else if name.contains("splitter") {
            let input_prio = value
                .get("input_priority")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or(Priority::None);

            let output_prio = value
                .get("output_priority")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or(Priority::None);

            Ok(Self::Splitter(FBSplitter {
                base,
                input_prio,
                output_prio,
            }))
        } else if name.contains("inserter") {
            if name.contains("long-handed") {
                base.throughput = 1.2;
                return Ok(Self::LongInserter(FBLongInserter { base }));
            }
            base.throughput = if name == "inserter" {
                0.83
            } else if name.contains("burner") {
                0.6
            } else {
                2.31
            };
            Ok(Self::Inserter(FBInserter { base }))
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
            Ok(Self::Assembler(FBAssembler { base }))
        } else {
            Err(format!("Invalid entity: ({})", name)).map_err(serde::de::Error::custom)
        }
    }
}

/// Some entities like splitters have their coordinates that are not integers.
/// This function snaps these coordinates to an integer coordinate system.
fn snap_to_grid(entities: &mut [FBEntity<f64>]) {
    for e in entities {
        match e {
            /* snap splitters to the grid as they are offset by 0.5 */
            FBEntity::Splitter(splitter) => {
                let shift_dir = splitter.base.direction.rotate(Rotation::Anticlockwise, 1);
                /* in Factorio blueprints the y-axis is inverted */
                let shift_dir = match shift_dir {
                    Direction::East => Direction::West,
                    Direction::West => Direction::East,
                    x => x,
                };
                splitter.base.shift(shift_dir, 0.5);
            }
            /* flip direction of inserters */
            FBEntity::Inserter(inserter) => {
                let dir = inserter.base.direction;
                inserter.base.direction = dir.flip();
            }
            /* flip direction of long inserters */
            FBEntity::LongInserter(inserter) => {
                let dir = inserter.base.direction;
                inserter.base.direction = dir.flip();
            }
            _ => (),
        }
    }
}

/// Constrains all the coordinates of the `FBEntity`s to be >= 0.
/// Additionally adds phantoms for entities that occupy multiple tiles like splitters or assemblers.
fn normalize_entities(entities: &[FBEntity<f64>]) -> Vec<FBEntity<i32>> {
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
            let base = FBBaseEntity {
                position,
                id: base.id,
                direction: base.direction,
                throughput: base.throughput,
            };
            match e {
                FBEntity::Belt(_) => FBEntity::Belt(FBBelt { base }),
                FBEntity::Underground(u) => FBEntity::Underground(FBUnderground {
                    base,
                    belt_type: u.belt_type,
                }),
                FBEntity::Splitter(s) => FBEntity::Splitter(FBSplitter {
                    base,
                    input_prio: s.input_prio,
                    output_prio: s.output_prio,
                }),
                FBEntity::SplitterPhantom(_) => {
                    FBEntity::SplitterPhantom(FBSplitterPhantom { base })
                }
                FBEntity::Inserter(_) => FBEntity::Inserter(FBInserter { base }),
                FBEntity::LongInserter(_) => FBEntity::LongInserter(FBLongInserter { base }),
                FBEntity::Assembler(_) => FBEntity::Assembler(FBAssembler { base }),
                FBEntity::AssemblerPhantom(_) => {
                    FBEntity::AssemblerPhantom(FBAssemblerPhantom { base })
                }
            }
        })
        .collect()
}

/// Parses a blueprint string, as exported from Factorio, to a list of `FBEntity`s
///
/// Unsupported entities, like power poles, are skipped.
pub fn string_to_entities(blueprint_string: &str) -> Result<Vec<FBEntity<i32>>> {
    let json = decompress_string(blueprint_string)?;
    let mut entities: Vec<_> = get_json_entities(json)?
        .into_iter()
        .flat_map(serde_json::from_value)
        .collect::<Vec<_>>();

    snap_to_grid(&mut entities);
    let mut entities = normalize_entities(&entities);

    // add splitter phantoms
    let phantoms = entities
        .iter()
        .filter_map(|&e| match e {
            FBEntity::Splitter(s) => Some(FBEntity::SplitterPhantom(s.get_phantom())),
            _ => None,
        })
        .collect::<Vec<_>>();
    entities.extend(phantoms);

    // add assembler phantoms
    let phantoms = entities
        .iter()
        .filter_map(|&e| match e {
            FBEntity::Assembler(a) => Some(a.get_phantoms()),
            _ => None,
        })
        .flatten()
        .map(FBEntity::AssemblerPhantom)
        .collect::<Vec<_>>();
    entities.extend(phantoms);
    Ok(entities)
}

/// Parses a file containing a blueprint string to a list of `FBEntity`s.
///
/// Unsupported entities, like power poles, are skipped.
pub fn file_to_entities(file: &str) -> Result<Vec<FBEntity<i32>>> {
    let blueprint_string = fs::read_to_string(file)?;
    string_to_entities(&blueprint_string)
}

#[cfg(test)]
mod tests {
    use crate::{
        entities::{BeltType, Priority},
        utils::Direction,
    };

    use super::*;
    use std::fs;
    fn get_belt_entities() -> Vec<FBEntity<i32>> {
        let blueprint_string = fs::read_to_string("tests/belts").unwrap();
        string_to_entities(&blueprint_string).unwrap()
    }

    fn get_assembly_entities() -> Vec<FBEntity<i32>> {
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
        assert_eq!(throughput, [4, 5, 1]);
    }

    #[test]
    fn belt_direction() {
        let entities = get_belt_entities();
        for e in entities {
            if let FBEntity::Belt(b) = e {
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
            if let FBEntity::Splitter(s) = e {
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
            if let FBEntity::Underground(u) = e {
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
            if let FBEntity::Inserter(i) = e {
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
            if let FBEntity::LongInserter(l) = e {
                assert_eq!(l.base.direction, Direction::South);
                assert_eq!(l.base.throughput, 1.2);
            }
        }
    }

    #[test]
    fn assembler() {
        let entities = get_assembly_entities();
        for e in &entities {
            if let FBEntity::Assembler(a) = e {
                assert_eq!(a.base.throughput, 1.25);
            }
        }
        println!("{:?}", &entities);
        assert_eq!(entities.len(), 9 + 3);
    }
}
