use anyhow::Context;
use serde::{de::Error, Deserialize, Deserializer};
use serde_json::Value;

use crate::{
    base_entity::BaseEntity,
    utils::{Direction, Position},
};

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

impl<T> Splitter<T> {
    pub fn get_base_mut(&mut self) -> &mut BaseEntity<T> {
        &mut self.base
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Entity<T> {
    Belt(Belt<T>),
    Underground(Underground<T>),
    Splitter(Splitter<T>),
}

impl<T> Entity<T> {
    fn as_inner(&self) -> &dyn EntityTrait<T> {
        match self {
            Self::Belt(x) => x as &dyn EntityTrait<T>,
            Self::Underground(x) => x as &dyn EntityTrait<T>,
            Self::Splitter(x) => x as &dyn EntityTrait<T>,
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
        } else {
            base.throughput = if name == "inserter" {
                0.83
            } else if name.contains("burner") {
                0.6
            } else if name.contains("long-handed") {
                1.2
            } else {
                2.31
            };
            panic!("Ony belt-related stuff is implemented");
        }
    }
}
