use std::ops::{Add, Sub};

use crate::utils::{Direction, Position};

#[derive(Debug, Clone, Copy)]
pub struct BaseEntity<T> {
    pub id: i32,
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
