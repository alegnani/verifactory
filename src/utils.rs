use std::{
    fs,
    ops::{Add, Sub},
};

use serde::Deserialize;
use serde_repr::Deserialize_repr;

use crate::{
    entities::{Entity, Priority},
    import::string_to_entities,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub struct Position<T> {
    pub x: T,
    pub y: T,
}

impl<T> Position<T>
where
    T: Add<Output = T> + Sub<Output = T> + Copy,
{
    pub fn shift(&self, direction: Direction, distance: T) -> Self {
        let x = self.x;
        let y = self.y;
        let (x, y) = match direction {
            Direction::North => (x, y + distance),
            Direction::East => (x + distance, y),
            Direction::South => (x, y - distance),
            Direction::West => (x - distance, y),
        };
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum Direction {
    North = 0,
    East = 2,
    South = 4,
    West = 6,
}

impl Direction {
    pub fn rotate(&self, direction: Rotation, amount: u8) -> Self {
        let incr = match direction {
            Rotation::Clockwise => 2,
            Rotation::Anticlockwise => 6,
        };
        let new_u8 = (*self as u8 + amount * incr) % 8;
        new_u8.into()
    }

    pub fn rotate_side(&self, side: Priority) -> Self {
        match side {
            Priority::None => *self,
            Priority::Left => self.rotate(Rotation::Anticlockwise, 1),
            Priority::Right => self.rotate(Rotation::Clockwise, 1),
        }
    }

    pub fn flip(&self) -> Self {
        self.rotate(Rotation::Clockwise, 2)
    }
}

impl From<u8> for Direction {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::North,
            2 => Self::East,
            4 => Self::South,
            6 => Self::West,
            _ => panic!("Direction is not in cardinal direction: ({})", value),
        }
    }
}

pub enum Rotation {
    Clockwise,
    Anticlockwise,
}

#[cfg(test)]
mod test {
    use super::*;
    use Direction::*;
    use Rotation::*;

    #[test]
    fn dir_rotate() {
        let north = North;
        let north2 = north.rotate(Clockwise, 4);
        let north3 = north.rotate(Anticlockwise, 8);
        assert_eq!(north, north2);
        assert_eq!(north, north3);

        let east = north.rotate(Clockwise, 1);
        assert_eq!(east, East);

        let south = east.rotate(Clockwise, 1);
        assert_eq!(south, South);

        let west = south.rotate(Clockwise, 1);
        assert_eq!(west, West);
    }
}

pub fn load_entities(file: &str) -> Vec<Entity<i32>> {
    let blueprint_string = fs::read_to_string(file).unwrap();
    string_to_entities(&blueprint_string).unwrap()
}
