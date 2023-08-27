use std::ops::{Add, Sub};

use crate::utils::{Direction, Position, Rotation};
use serde::Deserialize;

pub type EntityId = i32;

#[derive(Debug, Clone, Copy)]
pub struct BaseEntity<T> {
    pub id: EntityId,
    pub position: Position<T>,
    pub direction: Direction,
    pub throughput: f64,
}

impl<T> BaseEntity<T>
where
    T: Add<Output = T> + Sub<Output = T> + Copy,
{
    pub fn shift(&self, direction: Direction, distance: T) -> Self {
        let position = self.position.shift(direction, distance);
        Self { position, ..*self }
    }

    pub fn shift_mut(&mut self, direction: Direction, distance: T) {
        self.position = self.position.shift(direction, distance);
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Entity<T> {
    Belt(Belt<T>),
    Underground(Underground<T>),
    Splitter(Splitter<T>),
    Inserter(Inserter<T>),
    LongInserter(LongInserter<T>),
    Assembler(Assembler<T>),
}

impl<T> Entity<T> {
    pub fn get_base(&self) -> &BaseEntity<T> {
        match self {
            Self::Belt(b) => &b.base,
            Self::Underground(b) => &b.base,
            Self::Splitter(b) => &b.base,
            Self::Inserter(b) => &b.base,
            Self::LongInserter(b) => &b.base,
            Self::Assembler(b) => &b.base,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Belt<T> {
    pub base: BaseEntity<T>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BeltType {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy)]
pub struct Underground<T> {
    pub base: BaseEntity<T>,
    pub belt_type: BeltType,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    None,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
pub struct Splitter<T> {
    pub base: BaseEntity<T>,
    pub input_prio: Priority,
    pub output_prio: Priority,
}

impl Splitter<i32> {
    pub fn get_phantom(&self) -> Position<i32> {
        let base = self.base;
        let rotation = base.direction.rotate(Rotation::Anticlockwise, 1);
        base.position.shift(rotation, 1)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Inserter<T> {
    pub base: BaseEntity<T>,
}

impl Inserter<i32> {
    pub fn get_source(&self) -> Position<i32> {
        let base = self.base;
        self.base.position.shift(base.direction, -1)
    }

    pub fn get_destination(&self) -> Position<i32> {
        let base = self.base;
        self.base.position.shift(base.direction, 1)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LongInserter<T> {
    pub base: BaseEntity<T>,
}

impl LongInserter<i32> {
    pub fn get_source(&self) -> Position<i32> {
        let base = self.base;
        self.base.position.shift(base.direction, -2)
    }

    pub fn get_destination(&self) -> Position<i32> {
        let base = self.base;
        self.base.position.shift(base.direction, 2)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Assembler<T> {
    pub base: BaseEntity<T>,
}
