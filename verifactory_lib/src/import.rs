//! Utility functions to convert a Factorio blueprint string into a list of `FBEntity`s.
//! A description of the JSON representation of the blueprint string can be found [here](https://wiki.factorio.com/Blueprint_string_format).

use anyhow::{anyhow, bail, Context, Result};
use base64::engine::{general_purpose, Engine as _};
use inflate::inflate_bytes_zlib;
use serde::{de::Error, Deserialize};
use serde_json::Value;
use std::{collections::HashMap, fs};

use crate::{
    entities::*,
    utils::{Direction, FactorioVersion, Position, Rotation},
};

/// Decompresses the string such that it can be interpreted as a JSON.
#[tracing::instrument(skip(blueprint_string), fields(in_len = blueprint_string.len(), out_len), err)]
fn decompress_string(blueprint_string: &str) -> Result<String> {
    let skip_first_byte = &blueprint_string.as_bytes()[1..blueprint_string.len()];
    let base64_decoded = general_purpose::STANDARD
        .decode(skip_first_byte)
        .context("Decoding base64")?;
    let decoded =
        inflate_bytes_zlib(&base64_decoded).map_err(|s| anyhow!("zlib inflate error: {s}"))?;
    let s = String::from_utf8(decoded)
        .context("blueprint contains invalid characters (not valid UTF-8)")?;
    tracing::Span::current().record("out_len", s.len()); // print output length
    Ok(s)
}

/// Turns a JSON string into a list of JSON substrings, each representing an entity of the blueprint.
#[tracing::instrument(skip(data_json))]
fn get_blueprints(data_json: &str) -> Result<BookOrSingle> {
    let der = &mut serde_json::Deserializer::from_str(data_json);
    let out = serde_path_to_error::deserialize(der)?;
    Ok(out)
}

#[derive(Debug, Clone)]
enum BookOrSingle {
    Book(BlueprintBook),
    Single(BlueprintBookEntry),
}

impl<'de> Deserialize<'de> for BookOrSingle {
    fn deserialize<D>(deserializer: D) -> std::prelude::v1::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value: Value = Deserialize::deserialize(deserializer)?;

        if let Some(b) = value.get("blueprint_book") {
            Ok(BookOrSingle::Book(
                BlueprintBook::deserialize(b).map_err(Error::custom)?,
            ))
        } else {
            Ok(BookOrSingle::Single(
                BlueprintBookEntry::deserialize(value).map_err(Error::custom)?,
            ))
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct BlueprintBook {
    blueprints: Vec<BookOrSingle>,
    label: Option<String>,
}

type Index = u32;

#[derive(Debug, Clone, Deserialize)]
struct Blueprint {
    blueprint: BlueprintContent<f64>,
    #[serde(default)]
    index: Index,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum BlueprintBookEntry {
    Blueprint(Blueprint),
    /// Blueprint books can also contain upgrade planners, ...
    #[allow(unused)]
    // otherwise deserialization fails when non-blueprint (e.g. upgrade-planner)
    // idk why, shouldn't serde fallback to unit variant?
    Other(HashMap<String, Value>),
}

impl std::ops::DerefMut for Blueprint {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.blueprint
    }
}

impl std::ops::Deref for Blueprint {
    type Target = BlueprintContent<f64>;

    fn deref(&self) -> &Self::Target {
        &self.blueprint
    }
}

#[derive(Deserialize, Clone, Debug)]
struct BlueprintContent<T> {
    description: Option<String>,
    label: Option<String>,
    version: FactorioVersion,
    entities: Vec<FBEntity<T>>,
    #[allow(unused)]
    #[serde(flatten)]
    extra: HashMap<String, Value>,
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

/// Convert entities' direction values from pre-2.0 to post-2.0.
#[tracing::instrument(skip(entities))]
fn migrate_to_v2(entities: &mut [FBEntity<f64>]) {
    entities.iter_mut().for_each(|e| {
        let d = Direction::from(u8::from(e.get_base().direction) * 2);
        e.get_base_mut().direction = d;
    });
}

/// Parses a blueprint string, as exported from Factorio, to a list of `FBEntity`s
///
/// Unsupported entities, like power poles, are skipped.
#[tracing::instrument(skip(blueprint_string), fields(in_len = blueprint_string.len(), entity_count), err)]
pub fn string_to_entities(blueprint_string: &str) -> Result<Vec<FBEntity<i32>>> {
    let mut bos = string_to_book_or_single(blueprint_string)?;

    let entities = match &mut bos {
        BookOrSingle::Single(BlueprintBookEntry::Blueprint(blueprint)) => {
            tracing::debug!(
                blueprint.description,
                blueprint.label,
                "Got entities from blueprint"
            );
            &mut blueprint.blueprint.entities
        }
        BookOrSingle::Single(d) => bail!("Cannot get entities from a non-blueprint: {d:?}"),
        BookOrSingle::Book { .. } => bail!("Cannot get entities of a book"),
    };

    snap_to_grid(entities);
    tracing::debug!("Snapped entities to grid");
    let mut entities = normalize_entities(entities);
    tracing::debug!("Normalized entities");

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

    tracing::Span::current().record("entity_count", entities.len()); // print amount of entities
    Ok(entities)
}

#[tracing::instrument(skip(blueprint_string), fields(in_len = blueprint_string.len(), blueprint_count), err)]
fn string_to_book_or_single(blueprint_string: &str) -> Result<BookOrSingle> {
    let json = decompress_string(blueprint_string.trim_end())?;
    tracing::debug!(%json, "Decompressed string");
    let mut blueprints = get_blueprints(&json)?;
    tracing::debug!("Parsed blueprint(s)");

    /// recurse into a (maybe) book and fix every blueprint, if needed. TODO: Maybe
    /// generalize function later, e.g. for normalizing an entire book and stuff.
    fn recursive_maybe_fix_book_or_single(b: &mut BookOrSingle, indices: &mut Vec<usize>) {
        match b {
            BookOrSingle::Book(blueprint_book) => {
                tracing::debug!(blueprint_book.label, ?indices, "Enumerating book");
                for (idx, b) in blueprint_book.blueprints.iter_mut().enumerate() {
                    indices.push(idx);
                    recursive_maybe_fix_book_or_single(b, indices);
                    indices.pop();
                }
            }
            BookOrSingle::Single(BlueprintBookEntry::Blueprint(blueprint))
                if blueprint.version.major() < 2 =>
            {
                indices.push(blueprint.index as _);
                tracing::debug!(
                    %blueprint.version,
                    ?indices,
                    "Blueprint requires migration to Factorio 2.x format"
                );
                migrate_to_v2(&mut blueprint.entities);
                indices.pop();
            }
            _ => (),
        };
    }

    recursive_maybe_fix_book_or_single(&mut blueprints, &mut vec![]);

    let count = match &blueprints {
        BookOrSingle::Book(blueprint_book) => blueprint_book.blueprints.len(),
        BookOrSingle::Single(_) => 1,
    };
    tracing::Span::current().record("blueprint_count", count);

    Ok(blueprints)
}

/// Parses a file containing a blueprint string to a list of `FBEntity`s.
///
/// Unsupported entities, like power poles, are skipped.
#[tracing::instrument(err)]
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
    fn get_belt_entities() -> Vec<FBEntity<i32>> {
        let blueprint_string = include_str!("../tests/belts");
        string_to_entities(&blueprint_string).unwrap()
    }

    fn get_assembly_entities() -> Vec<FBEntity<i32>> {
        let blueprint_string = include_str!("../tests/inserter_assembler");
        string_to_entities(&blueprint_string).unwrap()
    }

    fn get_book() -> BlueprintBook {
        let book_string = include_str!("../tests/book-balancers");
        match string_to_book_or_single(book_string).unwrap() {
            BookOrSingle::Book(blueprint_book) => blueprint_book,
            BookOrSingle::Single(_) => {
                panic!("Was supposed to load a book, loaded a single blueprint instead")
            }
        }
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

    #[test]
    fn load_book() {
        let _b = get_book();
    }
}
