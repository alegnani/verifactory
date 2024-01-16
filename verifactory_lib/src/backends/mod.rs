//! Back-end used to convert the IR into a z3 model
mod model_entities;
mod model_graph;
mod proofs;

pub use self::proofs::{BlueprintProofEntity, ProofResult};

pub use model_graph::{
    belt_balancer_f, equal_drain_f, model_f, throughput_unlimited, universal_balancer, ModelFlags,
    ProofPrimitives,
};
