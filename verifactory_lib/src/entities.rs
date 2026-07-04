//! Definitions of entities that are part of a Factorio blueprint
//!
use crate::utils::{Direction, Position, Rotation};
use serde::{
    de::{Error, Unexpected, Visitor},
    Deserialize, Deserializer,
};
use serde_json::Value;
use std::{
    num::NonZeroI32,
    ops::{Add, Sub},
};

pub type EntityId = NonZeroI32;

/// Contains the subset of fields each entity possesses
#[derive(Debug, Copy, Clone, Deserialize)]
pub struct FBBaseEntity<T> {
    #[serde(rename = "entity_number")]
    pub id: EntityId,
    pub position: Position<T>,
    #[serde(default)]
    pub direction: Direction,
    #[serde(default)] // TODO: Maybe option instead?
    pub throughput: f64,
}

impl<T> FBBaseEntity<T>
where
    T: Add<Output = T> + Sub<Output = T> + Copy,
{
    /// Shifts the base entity by `distance` in the `direction`.
    pub fn shift(&mut self, direction: Direction, distance: T) {
        self.position = self.position.shift(direction, distance);
    }
}

/// Enum of possible entities that are supported by VeriFactory
///
/// The phantoms are used to populate the grid of entities with entities that are bigger than 1x1.
/// These include the splitter (2x1) and the assembler (3x3).
#[derive(Debug, Clone, Copy)]
pub enum FBEntity<T> {
    Belt(FBBelt<T>),
    Underground(FBUnderground<T>),
    Splitter(FBSplitter<T>),
    SplitterPhantom(FBSplitterPhantom<T>),
    Inserter(FBInserter<T>),
    LongInserter(FBLongInserter<T>),
    Assembler(FBAssembler<T>),
    AssemblerPhantom(FBAssemblerPhantom<T>),
}

impl<T> FBEntity<T> {
    /// Get a shared reference to the base entity of a `FBEntity<T>`.
    pub fn get_base(&self) -> &FBBaseEntity<T> {
        match self {
            Self::Belt(b) => &b.base,
            Self::Underground(b) => &b.base,
            Self::Splitter(b) => &b.base,
            Self::SplitterPhantom(b) => &b.base,
            Self::Inserter(b) => &b.base,
            Self::LongInserter(b) => &b.base,
            Self::Assembler(b) => &b.base,
            Self::AssemblerPhantom(b) => &b.base,
        }
    }

    /// Get an exclusive reference to the base entity of a `FBEntity<T>`.
    pub fn get_base_mut(&mut self) -> &mut FBBaseEntity<T> {
        match self {
            Self::Belt(b) => &mut b.base,
            Self::Underground(b) => &mut b.base,
            Self::Splitter(b) => &mut b.base,
            Self::SplitterPhantom(b) => &mut b.base,
            Self::Inserter(b) => &mut b.base,
            Self::LongInserter(b) => &mut b.base,
            Self::Assembler(b) => &mut b.base,
            Self::AssemblerPhantom(b) => &mut b.base,
        }
    }
}

/// Deserialization function turning each JSON string into a `FBEntity<f64>`.
impl<'de, T: Deserialize<'de>> Deserialize<'de> for FBEntity<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value: Value = Deserialize::deserialize(deserializer)?;
        let name = value
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or(Error::missing_field("name"))?;

        let belt_throughput = match name {
            n if n.contains("turbo") => 60.0,
            n if n.contains("express") => 45.0,
            n if n.contains("fast") => 30.0,
            _ => 15.0,
        };

        let (throughput, mut e) = if name.contains("transport-belt") {
            let belt = FBBelt::deserialize(value).map_err(Error::custom)?;
            (belt_throughput, Self::Belt(belt))
        } else if name.contains("underground-belt") {
            let ubelt = FBUnderground::deserialize(value).map_err(Error::custom)?;
            (belt_throughput, Self::Underground(ubelt))
        } else if name.contains("splitter") {
            let spli = FBSplitter::deserialize(value).map_err(Error::custom)?;
            (belt_throughput, Self::Splitter(spli))
        } else if name.contains("inserter") {
            if name.contains("long-handed") {
                (
                    1.2,
                    Self::LongInserter(FBLongInserter::deserialize(value).map_err(Error::custom)?),
                )
            } else {
                let t = if name == "inserter" {
                    0.83
                } else if name.contains("burner") {
                    0.6
                } else {
                    2.31
                };
                (
                    t,
                    Self::Inserter(FBInserter::deserialize(value).map_err(Error::custom)?),
                )
            }
        } else if name.contains("assembling-machine") {
            let tier = name
                .strip_prefix("assembling-machine-")
                .ok_or(Error::custom(
                    "Error whilst deserializing assembling machine tier",
                ))?;
            let t = match tier {
                "1" => 0.5,
                "2" => 0.75,
                "3" => 1.25,
                _ => panic!(),
            };
            let a = FBAssembler::deserialize(value).map_err(Error::custom)?;
            (t, Self::Assembler(a))
        } else {
            return Err(Error::invalid_value(
                Unexpected::Str(name),
                &"any belt part, inserters, assemblers",
            ));
        };

        e.get_base_mut().throughput = throughput;
        Ok(e)
    }
}

/// Belt entity
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct FBBelt<T> {
    #[serde(flatten)]
    pub base: FBBaseEntity<T>,
}

pub struct FBBeltRaw<T> {
    pub base: FBBaseEntity<T>,
}

/// Type of the underground belt. Either going into the ground, `Input`, or exiting, `Output`
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BeltType {
    Input,
    Output,
}

/// Underground belt entity
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct FBUnderground<T> {
    #[serde(flatten)]
    pub base: FBBaseEntity<T>,
    #[serde(rename = "type")]
    pub belt_type: BeltType,
}

/// Side priority for input or output of splitters
#[derive(Default, Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    #[default]
    #[serde(skip)] // omitted in serialization
    None,
    Left,
    Right,
}

/// Splitter entity
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct FBSplitter<T> {
    #[serde(flatten)]
    pub base: FBBaseEntity<T>,
    #[serde(default, rename = "input_priority")]
    pub input_prio: Priority,
    #[serde(default, rename = "output_priority")]
    pub output_prio: Priority,
}

impl FBSplitter<i32> {
    /// Get the phantom associated with a splitter entity.
    /// This is the left side of the splitter.
    pub fn get_phantom(&self) -> FBSplitterPhantom<i32> {
        let mut base = self.base;
        let rotation = base.direction.rotate(Rotation::Anticlockwise, 1);
        base.position = base.position.shift(rotation, 1);
        FBSplitterPhantom { base }
    }
}
/// Splitter phantom
#[derive(Debug, Clone, Copy)]
pub struct FBSplitterPhantom<T> {
    pub base: FBBaseEntity<T>,
}

/// Trait for getting source and destination cells of an inserter entity
pub trait InserterTrait {
    /// Get the source position of the inserter, from where items are picked up
    fn get_source(&self) -> Position<i32>;
    /// Get the destination position of the inserter, where items are placed
    fn get_destination(&self) -> Position<i32>;
}

/// Inserter entity
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct FBInserter<T> {
    #[serde(flatten)]
    pub base: FBBaseEntity<T>,
}

impl InserterTrait for FBInserter<i32> {
    fn get_source(&self) -> Position<i32> {
        self.base.position.shift(self.base.direction, -1)
    }

    fn get_destination(&self) -> Position<i32> {
        self.base.position.shift(self.base.direction, 1)
    }
}

/// Long inserter entity
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct FBLongInserter<T> {
    #[serde(flatten)]
    pub base: FBBaseEntity<T>,
}

impl InserterTrait for FBLongInserter<i32> {
    fn get_source(&self) -> Position<i32> {
        self.base.position.shift(self.base.direction, -2)
    }

    fn get_destination(&self) -> Position<i32> {
        self.base.position.shift(self.base.direction, 2)
    }
}

/// Assembler entity
#[derive(Deserialize, Debug, Clone, Copy)]
pub struct FBAssembler<T> {
    #[serde(flatten)]
    pub base: FBBaseEntity<T>,
}

impl FBAssembler<i32> {
    /// Get all the phantoms associated with the assembler entity.
    /// These are all the cells around the assembler entity as it's size is 3x3.
    pub fn get_phantoms(&self) -> Vec<FBAssemblerPhantom<i32>> {
        let center_base = self.base;
        let mut phantoms = vec![];
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let position = center_base.position + Position { x: dx, y: dy };
                let base = FBBaseEntity {
                    position,
                    ..center_base
                };
                phantoms.push(FBAssemblerPhantom { base });
            }
        }
        phantoms
    }
}

/// Assembler phantom
#[derive(Debug, Clone, Copy)]
pub struct FBAssemblerPhantom<T> {
    pub base: FBBaseEntity<T>,
}
