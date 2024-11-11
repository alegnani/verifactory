//! Various generic utilities

use std::ops::{Add, Neg, Sub};

use serde::Deserialize;
use serde_repr::Deserialize_repr;

use crate::entities::Priority;

/// Position of an entity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub struct Position<T> {
    pub x: T,
    pub y: T,
}

impl<T> Position<T>
where
    T: Add<Output = T> + Sub<Output = T> + Copy,
{
    /// Create new `Position` shifted in a given direction
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

impl<T> std::ops::Add for Position<T>
where
    T: Add<Output = T>,
{
    type Output = Position<T>;

    fn add(self, rhs: Self) -> Self::Output {
        Self::Output {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

/// Direction of an entity
///
/// Represented as a C-like enum as used in the Factorio blueprint JSON.
/// The odd numbers are used for directions that go diagonally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum Direction {
    North = 0,
    East = 4,
    South = 8,
    West = 12,
}

impl Direction {
    /// Returns a new `Direction` rotated in the given direction
    pub fn rotate(&self, direction: Rotation, amount: u8) -> Self {
        let incr = match direction {
            Rotation::Clockwise => 4,
            Rotation::Anticlockwise => 12,
        };
        let new_u8 = (*self as u8 + amount * incr) % 16;
        new_u8.into()
    }

    /// Returns a new `Direction` rotate to the given side
    pub fn rotate_side(&self, side: Priority) -> Self {
        match side {
            Priority::None => *self,
            Priority::Left => self.rotate(Rotation::Anticlockwise, 1),
            Priority::Right => self.rotate(Rotation::Clockwise, 1),
        }
    }

    /// Returns a new `Direction` pointing in the opposite direction
    pub fn flip(&self) -> Self {
        self.rotate(Rotation::Clockwise, 2)
    }
}

impl From<u8> for Direction {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::North,
            4 => Self::East,
            8 => Self::South,
            12 => Self::West,
            _ => panic!("Direction is not in cardinal direction: ({})", value),
        }
    }
}

/// Rotation direction enum
pub enum Rotation {
    Clockwise,
    Anticlockwise,
}

/// Generic enum indicating the side
///
/// Used in IR edges and IR splitters/mergers to indicate the priority of a given edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    None,
}

impl Side {
    /// Returns `true` if the side is a `None` value.
    pub fn is_none(&self) -> bool {
        *self == Self::None
    }
}

impl Neg for Side {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Self::None => Self::None,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

impl From<Priority> for Side {
    fn from(value: Priority) -> Self {
        match value {
            Priority::None => Self::None,
            Priority::Left => Self::Left,
            Priority::Right => Self::Right,
        }
    }
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
