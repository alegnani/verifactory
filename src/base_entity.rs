use std::ops::{Add, Sub};

use crate::utils::Direction;

struct BaseEntity<T> {
    x: T,
    y: T,
    direction: Direction,
}

impl<T> BaseEntity<T>
where
    T: Add<Output = T> + Sub<Output = T> + Copy,
{
    pub fn shift(&self, direction: Direction, distance: T) -> Self {
        let (x, y) = match direction {
            Direction::North => (self.x, self.y + distance),
            Direction::East => (self.x + distance, self.y),
            Direction::South => (self.x, self.y - distance),
            Direction::West => (self.x - distance, self.y),
        };
        let direction = self.direction;
        Self { x, y, direction }
    }
}
