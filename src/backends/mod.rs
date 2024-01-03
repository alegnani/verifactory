//! Back-end used to convert the IR into a z3 model
mod model_entities;
mod model_entities_blocked;
mod model_entities_relaxed;
mod model_graph;
mod proofs;

pub use self::proofs::{Printable, Z3Proofs};
// pub use self::z3::Z3Backend;

pub use model_graph::{
    belt_balancer_f, equal_drain_f, model_f, throughput_unlimited, ModelType, ProofPrimitives,
};
