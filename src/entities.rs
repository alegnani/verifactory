use crate::base_entity::BaseEntity;
use serde::Deserialize;

pub trait EntityTrait<T> {
    fn get_base(&self) -> &BaseEntity<T>;
}

impl<T> EntityTrait<T> for Belt<T> {
    fn get_base(&self) -> &BaseEntity<T> {
        &self.base
    }
}

impl<T> EntityTrait<T> for Underground<T> {
    fn get_base(&self) -> &BaseEntity<T> {
        &self.base
    }
}

impl<T> EntityTrait<T> for Splitter<T> {
    fn get_base(&self) -> &BaseEntity<T> {
        &self.base
    }
}

impl<T> EntityTrait<T> for Inserter<T> {
    fn get_base(&self) -> &BaseEntity<T> {
        &self.base
    }
}

impl<T> EntityTrait<T> for LongInserter<T> {
    fn get_base(&self) -> &BaseEntity<T> {
        &self.base
    }
}

impl<T> EntityTrait<T> for Assembler<T> {
    fn get_base(&self) -> &BaseEntity<T> {
        &self.base
    }
}

impl<T> Splitter<T> {
    pub fn get_base_mut(&mut self) -> &mut BaseEntity<T> {
        &mut self.base
    }
}

impl<T> Inserter<T> {
    pub fn get_base_mut(&mut self) -> &mut BaseEntity<T> {
        &mut self.base
    }
}

impl<T> LongInserter<T> {
    pub fn get_base_mut(&mut self) -> &mut BaseEntity<T> {
        &mut self.base
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
    fn as_inner(&self) -> &dyn EntityTrait<T> {
        match self {
            Self::Belt(x) => x as &dyn EntityTrait<T>,
            Self::Underground(x) => x as &dyn EntityTrait<T>,
            Self::Splitter(x) => x as &dyn EntityTrait<T>,
            Self::Inserter(x) => x as &dyn EntityTrait<T>,
            Self::LongInserter(x) => x as &dyn EntityTrait<T>,
            Self::Assembler(x) => x as &dyn EntityTrait<T>,
        }
    }
}

impl<T> EntityTrait<T> for Entity<T> {
    fn get_base(&self) -> &BaseEntity<T> {
        self.as_inner().get_base()
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

#[derive(Debug, Clone, Copy)]
pub struct Inserter<T> {
    pub base: BaseEntity<T>,
}

#[derive(Debug, Clone, Copy)]
pub struct LongInserter<T> {
    pub base: BaseEntity<T>,
}

#[derive(Debug, Clone, Copy)]
pub struct Assembler<T> {
    pub base: BaseEntity<T>,
}
