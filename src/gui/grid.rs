use std::f32::consts::PI;

use egui::{Color32, Image, Pos2, Rect, Sense, Vec2};

use crate::{
    entities::{BeltType, Entity, EntityId},
    utils::{Direction, Position},
};

use super::app::MyApp;

impl MyApp {
    pub fn entities_to_grid(entities: Vec<Entity<i32>>) -> Vec<Vec<Option<Entity<i32>>>> {
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

    fn draw_io(&self, ui: &mut egui::Ui, rect: Rect, entity: &Entity<i32>) {
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
        // draw the arrow
        ui.put(rect, img);
    }

    fn draw_selection(&self, ui: &mut egui::Ui, rect: Rect) {
        let img = Image::new(egui::include_image!("../../imgs/selection.svg"))
            .tint(Color32::from_rgb(255, 127, 80))
            .fit_to_exact_size(Vec2::splat(self.grid_settings.size as f32));
        ui.put(rect, img);
    }

    fn get_entity_img(entity: &Entity<i32>) -> Image {
        let base = entity.get_base();
        let rotation = base.direction as u8 as f32 * PI / 4.;
        match entity {
            Entity::Splitter(_) => match base.direction {
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
                Entity::Underground(u) if u.belt_type == BeltType::Input => Image::new(
                    egui::include_image!("../../imgs/yellow_underground_input.png"),
                ),
                Entity::Underground(_) => Image::new(egui::include_image!(
                    "../../imgs/yellow_underground_output.png"
                )),
                Entity::Belt(_) => {
                    Image::new(egui::include_image!("../../imgs/yellow_belt_straight.png"))
                }
                _ => panic!(),
            }
            .rotate(rotation, Vec2::splat(0.5)),
        }
        .sense(Sense::click())
    }

    fn draw_img(&self, ui: &mut egui::Ui, entity: &Entity<i32>) -> Option<Entity<i32>> {
        let s = &self.grid_settings;
        let base = entity.get_base();

        let mut pos_rect = self.get_grid_rect(base.position);
        let img = Self::get_entity_img(entity);
        if let Entity::Splitter(_) = entity {
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

        let ret = if ui.put(pos_rect, img).clicked() {
            Some(*entity)
        } else {
            None
        };
        match self.selection {
            Some(sel) if sel.get_base().id == base.id => self.draw_selection(ui, pos_rect),
            _ => (),
        }
        self.draw_io(ui, pos_rect, entity);
        ret
    }
}
