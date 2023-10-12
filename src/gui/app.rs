use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use egui_file::FileDialog;
use z3::SatResult;

use crate::{
    backends::{Z3Backend, Z3Proofs},
    compiler::Compiler,
    entities::{Entity, EntityId},
    import::string_to_entities,
    ir::{FlowGraph, FlowGraphFun, Node},
    utils::load_entities,
};

use super::menu::BlueprintString;

#[derive(Default)]
pub struct FileState {
    pub opened_file: Option<PathBuf>,
    pub open_file_dialog: Option<FileDialog>,
}

pub struct GridSettings {
    pub max_y: i32,
    pub y_offset: i32,
    pub x_offset: i32,
    pub size: i32,
}

impl GridSettings {
    pub fn from(grid: &Vec<Vec<Option<Entity<i32>>>>) -> Self {
        Self {
            max_y: grid.len() as i32 + 1,
            y_offset: 0,
            x_offset: 0,
            size: 50,
        }
    }
}

#[derive(Default)]
pub struct IOState {
    pub input_candidates: HashSet<EntityId>,
    pub output_candidates: HashSet<EntityId>,
    pub input_entities: HashSet<EntityId>,
    pub output_entities: HashSet<EntityId>,
}

impl IOState {
    pub fn from_graph(graph: &FlowGraph) -> Self {
        let mut input_candidates = HashSet::new();
        let mut output_candidates = HashSet::new();
        for node in graph.node_weights() {
            match node {
                Node::Input(e) => input_candidates.insert(e.id),
                Node::Output(e) => output_candidates.insert(e.id),
                _ => continue,
            };
        }
        let input_entities = input_candidates.clone();
        let output_entities = output_candidates.clone();
        Self {
            input_candidates,
            output_candidates,
            input_entities,
            output_entities,
        }
    }
}

#[derive(Default)]
pub struct ProofState {
    balancer: Option<SatResult>,
}

pub struct MyApp {
    pub grid: Vec<Vec<Option<Entity<i32>>>>,
    pub grid_settings: GridSettings,
    pub io_state: IOState,
    pub open_file_state: FileState,
    pub proof_state: ProofState,
    pub graph: FlowGraph,
    pub selection: Option<Entity<i32>>,
    pub blueprint_string: BlueprintString,
}

impl Default for MyApp {
    fn default() -> Self {
        let grid = vec![vec![]];
        let grid_settings = GridSettings::from(&grid);
        let io_state = IOState::default();
        let open_file_state = FileState::default();
        let proof_state = ProofState::default();
        let graph = FlowGraph::default();
        let selection = None;
        let blueprint_string = BlueprintString::default();
        Self {
            grid,
            grid_settings,
            io_state,
            proof_state,
            open_file_state,
            graph,
            selection,
            blueprint_string,
        }
    }
}

impl MyApp {
    fn generate_z3(&self) -> Z3Backend {
        let mut graph = self.graph.clone();
        let io_state = &self.io_state;
        let removed_inputs = io_state
            .input_candidates
            .difference(&io_state.input_entities);
        let removed_outputs = io_state
            .output_candidates
            .difference(&io_state.output_entities);

        let removed = removed_inputs
            .chain(removed_outputs)
            .cloned()
            .collect::<Vec<_>>();

        println!("Remove list: {:?}", removed);

        graph.simplify(&removed);
        Z3Backend::new(graph)
    }

    pub fn load_file(&mut self, file: PathBuf) {
        self.open_file_state.opened_file = Some(file.clone());
        let blueprint_string = std::fs::read_to_string(file).unwrap();
        self.load_string(&blueprint_string);
    }

    pub fn load_string(&mut self, blueprint: &str) {
        let loaded_entities = string_to_entities(blueprint).unwrap();
        self.grid = Self::entities_to_grid(loaded_entities.clone());
        self.grid_settings = GridSettings::from(&self.grid);

        self.graph = Compiler::new(loaded_entities).create_graph();
        self.graph.simplify(&[]);
        self.graph.to_svg("main.svg");
        self.io_state = IOState::from_graph(&self.graph);
        self.proof_state = ProofState::default();
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.draw_menu(ctx);

        egui::TopBottomPanel::top("blueprint_panel").show(ctx, |ui| {
            let s = &self.grid_settings;
            let dimensions = (s.size * s.max_y) as f32;
            ui.set_height_range(dimensions..=dimensions);
            ui.heading("Blueprint");
            self.draw_grid(ui);
        });

        let io_state = &mut self.io_state;
        if let Some(sel) = self.selection {
            egui::SidePanel::right("right").show(ctx, |ui| {
                let base = sel.get_base();
                let id = base.id;
                ui.heading("Entity information");
                ui.separator();
                ui.label(format!("Entity ID: {}", id));
                ui.label(format!("Throughput: {}/s", base.throughput as i32));
                ui.horizontal(|ui| {
                    if io_state.input_entities.contains(&id) {
                        ui.horizontal(|ui| {
                            ui.label("Selected as blueprint input");
                            if ui.button("Remove from input").clicked() {
                                io_state.input_entities.remove(&id);
                            }
                        });
                    } else if io_state.input_candidates.contains(&id) {
                        ui.label("Can be selected as blueprint input");
                        if ui.button("Select as input").clicked() {
                            io_state.input_entities.insert(id);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if io_state.output_entities.contains(&id) {
                        ui.label("Selected as blueprint output");
                        if ui.button("Remove from output").clicked() {
                            io_state.output_entities.remove(&id);
                        }
                    } else if io_state.output_candidates.contains(&id) {
                        ui.label("Can be selected as blueprint output");
                        if ui.button("Select as output").clicked() {
                            io_state.output_entities.insert(id);
                        }
                    }
                });
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Proofs");
            ui.separator();
            ui.heading("Is it a belt-balancer?");
            ui.horizontal(|ui| {
                if ui.button("Prove").clicked() {
                    let z3 = self.generate_z3();
                    self.proof_state.balancer = Some(z3.is_balancer());
                }
                if let Some(proof_res) = self.proof_state.balancer {
                    ui.label(format!("Proof result: {:?}", proof_res));
                }
            });
        });
    }
}
