use std::f32::consts::PI;

use egui::{Color32, Image, Pos2, Rect, Sense, Vec2};

use crate::{
    entities::{BeltType, FBBelt, FBEntity, FBSplitter, Priority},
    frontend::RelMap,
    utils::{Direction, Position, Rotation},
};

use super::app::{EntityGrid, MyApp};

trait ShrinkDirection {
    fn shrink_dir(&self, side: Direction, amount: f32) -> Self;
}

impl ShrinkDirection for Rect {
    fn shrink_dir(&self, side: Direction, amount: f32) -> Self {
        let mut rect = *self;
        match side {
            Direction::North => rect.min += Vec2::new(0., amount),
            Direction::East => rect.max += Vec2::new(-amount, 0.),
            Direction::South => rect.max += Vec2::new(0., -amount),
            Direction::West => rect.min += Vec2::new(amount, 0.),
        }
        rect
    }
}

fn prio_rect(splitter: &FBSplitter<i32>, rect: Rect, size: f32) -> Vec<Rect> {
    let dir = splitter.base.direction;
    let mut vec = vec![];
    match splitter.input_prio {
        Priority::None => (),
        x => {
            let rot = splitter.base.direction.rotate_side(x);
            let n_rect = rect
                .shrink_dir(dir, size / 2.)
                .shrink_dir(rot.flip(), 5. / 4. * size)
                .shrink_dir(rot, size / 4.);
            vec.push(n_rect);
        }
    }
    match splitter.output_prio {
        Priority::None => (),
        x => {
            let rot = splitter.base.direction.rotate_side(x);
            let n_rect = rect
                .shrink_dir(dir.flip(), size / 2.)
                .shrink_dir(rot.flip(), 5. / 4. * size)
                .shrink_dir(rot, size / 4.);
            vec.push(n_rect);
        }
    }
    vec
}

fn determine_belt_rotation(
    belt: &FBBelt<i32>,
    feeds_from_map: &RelMap<Position<i32>>,
    grid: &EntityGrid,
) -> Option<Rotation> {
    let feeds_from = feeds_from_map.get(&belt.base.position);
    feeds_from.and_then(|f| {
        if f.len() != 1 {
            None
        } else {
            let pos = f.iter().next().unwrap();
            /* TODO: this is very inefficient, maybe keep a HashMap<Position<i32>, Entity<i32>> in MyApp */
            let feeding_entity = grid
                .iter()
                .flatten()
                .flatten()
                .find(|&e| e.get_base().position == *pos)
                .unwrap();
            let feeding_dir = feeding_entity.get_base().direction;
            let belt_dir = belt.base.direction;
            if belt_dir == feeding_dir.rotate(Rotation::Anticlockwise, 1) {
                Some(Rotation::Anticlockwise)
            } else if belt_dir == feeding_dir.rotate(Rotation::Clockwise, 1) {
                Some(Rotation::Clockwise)
            } else {
                None
            }
        }
    })
}

impl MyApp {
    pub fn entities_to_grid(entities: Vec<FBEntity<i32>>) -> EntityGrid {
        let (max_x, max_y) = entities
            .iter()
            .map(|e| {
                let position = e.get_base().position;
                (position.x, position.y)
            })
            .fold((0, 0), |(x_old, y_old), (x, y)| {
                (x_old.max(x), y_old.max(y))
            });
        let mut grid = vec![vec![None; (max_x + 1) as usize]; (max_y + 1) as usize];
        for entity in entities {
            let pos = entity.get_base().position;
            grid[pos.y as usize][pos.x as usize] = Some(entity);
        }
        grid
    }

    pub fn draw_grid(&mut self, ui: &mut egui::Ui) {
        for entity in self.grid.iter().flatten().flatten() {
            let selection = self.draw_img(ui, entity);
            if selection.is_some() {
                self.selection = selection;
            }
        }
    }

    fn get_grid_rect(&self, position: Position<i32>) -> Rect {
        let s = &self.grid_settings;
        let x_origin = s.x_offset + position.x * s.size;
        let y_origin = s.y_offset + (s.max_y - position.y) * s.size;
        Rect {
            min: Pos2 {
                x: x_origin as f32,
                y: y_origin as f32,
            },
            max: Pos2 {
                x: (x_origin + s.size) as f32,
                y: (y_origin + s.size) as f32,
            },
        }
    }

    fn draw_io(&self, ui: &mut egui::Ui, mut rect: Rect, entity: &FBEntity<i32>) {
        let base = entity.get_base();
        let id = base.id;
        let is_input = self.io_state.input_entities.contains(&id);
        let is_output = self.io_state.output_entities.contains(&id);
        if !(is_input || is_output) {
            return;
        }
        let rotation = base.direction as u8 as f32 * PI / 4.;
        let color = if is_input {
            Color32::LIGHT_GREEN
        } else {
            Color32::from_rgb(191, 64, 191) // PURPLE
        };
        let img = Image::new(egui::include_image!("../../imgs/arrow.svg"))
            .rotate(rotation, Vec2::splat(0.5))
            .tint(color)
            .fit_to_fraction(Vec2::splat(0.7));
        /* if the entity is a splitter force the arrow to be drawn in the middle */
        if let FBEntity::Splitter(s) = entity {
            let size = self.grid_settings.size as f32;
            let rot = s
                .base
                .direction
                .rotate(crate::utils::Rotation::Clockwise, 1);
            rect = rect
                .shrink_dir(rot, size / 2.)
                .shrink_dir(rot.flip(), size / 2.);
        }
        // draw the arrow
        ui.put(rect, img);
    }

    fn draw_prio(&self, ui: &mut egui::Ui, rect: Rect, splitter: &FBSplitter<i32>) {
        let base = splitter.base;
        let rotation = base.direction as u8 as f32 * PI / 4.;
        let color = Color32::YELLOW;
        let img = Image::new(egui::include_image!("../../imgs/arrow.svg"))
            .rotate(rotation, Vec2::splat(0.5))
            .tint(color);
        let size = self.grid_settings.size as f32;
        for p_rect in prio_rect(splitter, rect, size) {
            ui.put(p_rect, img.clone());
        }
    }

    fn draw_selection(&self, ui: &mut egui::Ui, rect: Rect) {
        let img = Image::new(egui::include_image!("../../imgs/selection.svg"))
            .tint(Color32::from_rgb(255, 127, 80))
            .fit_to_exact_size(Vec2::splat(self.grid_settings.size as f32));
        ui.put(rect, img);
    }

    fn get_entity_img(entity: &FBEntity<i32>, belt_rotation: Option<Rotation>) -> Image {
        let base = entity.get_base();
        let rotation = base.direction as u8 as f32 * PI / 4.;
        match entity {
            FBEntity::Splitter(_) => match base.direction {
                Direction::North => {
                    Image::new(egui::include_image!("../../imgs/yellow_splitter_0.png"))
                }
                Direction::East => {
                    Image::new(egui::include_image!("../../imgs/yellow_splitter_2.png"))
                }
                Direction::South => {
                    Image::new(egui::include_image!("../../imgs/yellow_splitter_4.png"))
                }
                Direction::West => {
                    Image::new(egui::include_image!("../../imgs/yellow_splitter_6.png"))
                }
            },
            x => match x {
                FBEntity::Underground(u) if u.belt_type == BeltType::Input => Image::new(
                    egui::include_image!("../../imgs/yellow_underground_input.png"),
                ),
                FBEntity::Underground(_) => Image::new(egui::include_image!(
                    "../../imgs/yellow_underground_output.png"
                )),
                FBEntity::Belt(_) => match belt_rotation {
                    None => Image::new(egui::include_image!("../../imgs/yellow_belt_straight.png")),
                    Some(Rotation::Anticlockwise) => {
                        Image::new(egui::include_image!("../../imgs/yellow_belt_anticlock.png"))
                    }
                    Some(Rotation::Clockwise) => {
                        Image::new(egui::include_image!("../../imgs/yellow_belt_clock.png"))
                    }
                },
                _ => panic!(),
            }
            .rotate(rotation, Vec2::splat(0.5)),
        }
        .sense(Sense::click())
    }

    fn draw_img(&self, ui: &mut egui::Ui, entity: &FBEntity<i32>) -> Option<FBEntity<i32>> {
        let s = &self.grid_settings;
        let base = entity.get_base();

        let mut pos_rect = self.get_grid_rect(base.position);
        let mut rotation = None;
        match entity {
            FBEntity::Splitter(_) => {
                let size = s.size as f32;
                pos_rect.min += match base.direction {
                    Direction::North => Vec2 { x: -size, y: 0. },
                    Direction::East => Vec2 { x: 0., y: -size },
                    _ => Vec2 { x: 0., y: 0. },
                };
                pos_rect.max += match base.direction {
                    Direction::South => Vec2 { x: size, y: 0. },
                    Direction::West => Vec2 { x: 0., y: size },
                    _ => Vec2 { x: 0., y: 0. },
                };
            }
            FBEntity::Belt(b) => {
                rotation = determine_belt_rotation(b, &self.feeds_from, &self.grid)
            }
            FBEntity::Underground(_) => (),
            _ => return None,
        }
        let img = Self::get_entity_img(entity, rotation);

        let ret = if ui.put(pos_rect, img).clicked() {
            Some(*entity)
        } else {
            None
        };
        match self.selection {
            Some(sel) if sel.get_base().id == base.id => self.draw_selection(ui, pos_rect),
            _ => (),
        }
        if let FBEntity::Splitter(s) = entity {
            self.draw_prio(ui, pos_rect, s);
        }
        self.draw_io(ui, pos_rect, entity);
        ret
    }
}
