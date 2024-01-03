//! Definitions of entities that are part of a Factorio blueprint
//!
use crate::utils::{Direction, Position, Rotation};
use serde::Deserialize;
use std::ops::{Add, Sub};

pub type EntityId = i32;

/// Contains the subset of fields each entity possesses
#[derive(Debug, Clone, Copy)]
pub struct FBBaseEntity<T> {
    pub id: EntityId,
    pub position: Position<T>,
    pub direction: Direction,
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

/// Enum of possible entities that are supported by Factorio Verify
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
}

/// Belt entity
#[derive(Debug, Clone, Copy)]
pub struct FBBelt<T> {
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
#[derive(Debug, Clone, Copy)]
pub struct FBUnderground<T> {
    pub base: FBBaseEntity<T>,
    pub belt_type: BeltType,
}

/// Side priority for input or output of splitters
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    None,
    Left,
    Right,
}

/// Splitter entity
#[derive(Debug, Clone, Copy)]
pub struct FBSplitter<T> {
    pub base: FBBaseEntity<T>,
    pub input_prio: Priority,
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
#[derive(Debug, Clone, Copy)]
pub struct FBInserter<T> {
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
#[derive(Debug, Clone, Copy)]
pub struct FBLongInserter<T> {
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
#[derive(Debug, Clone, Copy)]
pub struct FBAssembler<T> {
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
