use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use egui::{Align2, Direction, Event};
use egui_file::FileDialog;
use egui_toast::{Toast, ToastOptions, Toasts};

use verifactory_lib::{
    backends::{
        belt_balancer_f, equal_drain_f, throughput_unlimited, universal_balancer,
        BlueprintProofEntity, ModelFlags, ProofResult,
    },
    entities::{EntityId, FBEntity},
    frontend::{Compiler, RelMap},
    import::string_to_entities,
    ir::{CoalesceStrength, FlowGraph, FlowGraphFun, Node, Reversable},
    utils::Position,
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
    pub fn from(grid: &EntityGrid) -> Self {
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
    balancer: Option<ProofResult>,
    equal_drain: Option<ProofResult>,
    throughput_unlimited: Option<ProofResult>,
    universal: Option<ProofResult>,
}

pub type EntityGrid = Vec<Vec<Option<FBEntity<i32>>>>;
pub struct MyApp {
    pub grid: EntityGrid,
    pub grid_settings: GridSettings,
    pub io_state: IOState,
    pub open_file_state: FileState,
    pub proof_state: ProofState,
    pub graph: FlowGraph,
    pub selection: Option<FBEntity<i32>>,
    pub blueprint_string: BlueprintString,
    pub feeds_from: RelMap<Position<i32>>,
    pub show_error: bool,
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
        let feeds_from = HashMap::new();
        let show_error = false;
        Self {
            grid,
            grid_settings,
            io_state,
            proof_state,
            open_file_state,
            graph,
            selection,
            blueprint_string,
            feeds_from,
            show_error,
        }
    }
}

impl MyApp {
    fn generate_graph(&self, reversed: bool) -> FlowGraph {
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

        graph.simplify(&removed, CoalesceStrength::Aggressive);
        if reversed {
            Reversable::reverse(&graph)
        } else {
            graph
        }
    }

    pub fn load_file(&mut self, file: PathBuf) -> anyhow::Result<()> {
        let blueprint_string = std::fs::read_to_string(file.clone())?;
        self.open_file_state.opened_file = Some(file);
        self.load_string(&blueprint_string)
    }

    pub fn load_string(&mut self, blueprint: &str) -> anyhow::Result<()> {
        let loaded_entities = string_to_entities(blueprint)?;
        self.grid = Self::entities_to_grid(loaded_entities.clone());
        self.grid_settings = GridSettings::from(&self.grid);

        let compiler = Compiler::new(loaded_entities);
        self.feeds_from = compiler.feeds_from.clone();
        self.graph = compiler.create_graph();
        self.graph.simplify(&[], CoalesceStrength::Lossless);
        self.io_state = IOState::from_graph(&self.graph);
        self.proof_state = ProofState::default();
        Ok(())
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Set up toast notifications in the top right
        let mut toasts = Toasts::new()
            .anchor(Align2::RIGHT_TOP, (-10.0, 10.0))
            .direction(Direction::TopDown);

        self.draw_menu(ctx);

        // Handle Ctrl+V to load blueprint from clipboard
        ctx.input(|i| {
            let pasted_string = i.events.iter().find_map(|e| match e {
                Event::Paste(s) => Some(s),
                _ => None,
            });
            if let Some(pasted_string) = pasted_string {
                if self.load_string(pasted_string).is_err() {
                    toasts.add(Toast {
                        text: "Failed to load blueprint from clipboard!".into(),
                        kind: egui_toast::ToastKind::Error,
                        options: ToastOptions::default().duration_in_seconds(10.0),
                    });
                }
            }
        });

        toasts.show(ctx);

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

        if self.show_error {
            egui::Window::new("Error").title_bar(false).show(ctx, |ui| {
                ui.heading("Error whilst loading blueprint!");
                ui.label("Blueprint string is either malformed or uses non supported entities.");
                if ui.button("Close").clicked() {
                    self.show_error = false;
                }
            });
        }

        egui::TopBottomPanel::top("proof_panel").show(ctx, |ui| {
            ui.heading("Proofs");
            ui.separator();

            // TODO: figure out lifetimes and fix code duplication
            ui.heading("Is it a belt-balancer?");
            ui.horizontal(|ui| {
                if ui.button("Prove").clicked() {
                    let graph = self.generate_graph(false);
                    let mut proof = BlueprintProofEntity::new(graph);
                    let res = proof.model(belt_balancer_f, ModelFlags::empty());
                    self.proof_state.balancer = Some(res);
                }
                if let Some(proof_res) = self.proof_state.balancer {
                    ui.label(format!("Proof result: {}", proof_res));
                }
            });

            ui.label("\n");

            ui.heading("Is it an equal drain belt-balancer (assumes it is a belt-balancer)?");
            ui.horizontal(|ui| {
                if ui.button("Prove").clicked() {
                    let graph = self.generate_graph(true);
                    let mut proof = BlueprintProofEntity::new(graph);
                    let res = proof.model(equal_drain_f, ModelFlags::empty());
                    self.proof_state.equal_drain = Some(res);
                }
                if let Some(proof_res) = self.proof_state.equal_drain {
                    ui.label(format!("Proof result: {}", proof_res));
                }
            });

            ui.label("\n");

            ui.heading(
                "Is it a throughput unlimited belt-balancer (assumes it is a belt-balancer)?",
            );
            ui.horizontal(|ui| {
                if ui.button("Prove").clicked() {
                    let graph = self.generate_graph(false);
                    let mut proof = BlueprintProofEntity::new(graph);
                    let entities = self.grid.iter().flatten().flatten().cloned().collect();
                    let res = proof.model(throughput_unlimited(entities), ModelFlags::Relaxed);
                    self.proof_state.throughput_unlimited = Some(res);
                }
                if let Some(proof_res) = self.proof_state.throughput_unlimited {
                    ui.label(format!("Proof result: {}", proof_res));
                }
            });
            ui.label("\n");

            ui.heading("Is it a universal belt-balancer?");
            ui.horizontal(|ui| {
                if ui.button("Prove").clicked() {
                    let graph = self.generate_graph(false);
                    let mut proof = BlueprintProofEntity::new(graph);
                    let res = proof.model(universal_balancer, ModelFlags::Blocked);
                    self.proof_state.universal = Some(res);
                }
                if let Some(proof_res) = self.proof_state.universal {
                    ui.label(format!("Proof result: {}", proof_res));
                }
            });

            ui.label("\n");

            if ui.button("Save svg").clicked() {
                self.generate_graph(false).to_svg("out.svg").unwrap();
            }
            if ui.button("Save reversed svg").clicked() {
                self.generate_graph(true).to_svg("out.svg").unwrap();
            }
            ui.label("\n");
        });

        /* Show features and current state of project */
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Current state of the project");
            ui.label("- Currently only supports belts, underground belts and splitters (with priorities).\n  \
            Side-loading and other constructs taking advantage of a belt being split into two lanes is currently WIP.\n  \
            Read: The analysis will *definetely* be wrong.");
            ui.label("- All belts show as yellow but they are still modelled correctly.\n  \
            Clicking on a belt will show its real throughput (15 for yellow, 30 for red, 45 for blue.");
            ui.label("- Big blueprints won't fit on the screen.\n  \
            Use *View > Decrease blueprint size* to zoom out. \
            A better, zoomable and movable, canvas is WIP.");
            ui.label("- VeriFactory can prove much more than the automatic proofs above.\n  \
            A custom language to specify own properties is WIP.");
            ui.label("\n  Thank you for testing VeriFactory and have fun.\n  The factory must grow!");
        });
    }
}
