use std::fmt::Display;

use z3::{ast::Bool, Config, Context, SatResult};

use crate::ir::FlowGraph;

use super::{model_f, ModelFlags, ProofPrimitives};

#[derive(Debug, Clone, Copy)]
pub enum ProofResult {
    Unknown,
    Sat,
    Unsat,
}

impl ProofResult {
    pub fn not(&self) -> Self {
        match self {
            ProofResult::Sat => ProofResult::Unsat,
            ProofResult::Unsat => ProofResult::Sat,
            ProofResult::Unknown => ProofResult::Unknown,
        }
    }
}

impl From<SatResult> for ProofResult {
    fn from(value: SatResult) -> Self {
        match value {
            SatResult::Unknown => ProofResult::Unknown,
            SatResult::Unsat => ProofResult::Unsat,
            SatResult::Sat => ProofResult::Sat,
        }
    }
}

impl Display for ProofResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Sat => "Yes",
            Self::Unsat => "No",
            Self::Unknown => "Unknown",
        };
        write!(f, "{}", s)
    }
}

pub struct BlueprintProofEntity {
    _cfg: Config,
    ctx: Context,
    graph: FlowGraph,
    result: Option<ProofResult>,
}

impl BlueprintProofEntity {
    pub fn new(graph: FlowGraph) -> Self {
        let _cfg = Config::new();
        let ctx = Context::new(&_cfg);
        Self {
            _cfg,
            ctx,
            graph,
            result: None,
        }
    }

    pub fn model<'a, F>(&'a mut self, f: F, flags: ModelFlags) -> ProofResult
    where
        F: FnOnce(ProofPrimitives<'a>) -> Bool<'a>,
    {
        let res = model_f(&self.graph, &self.ctx, f, flags).into();
        self.result = Some(res);
        res
    }

    pub fn result(&self) -> Option<ProofResult> {
        self.result
    }
}

// TODO: decide what to do with these tests
// #[cfg(test)]
// mod test {
//     use crate::{
//         frontend::Compiler,
//         import::file_to_entities,
//         ir::{CoalesceStrength::Aggressive, FlowGraphFun},
//     };

//     use super::*;

//     #[test]
//     fn balancer_3_2() {
//         let entities = file_to_entities("tests/3-2").unwrap();
//         let mut graph = Compiler::new(entities).create_graph();
//         graph.simplify(&[3], Aggressive);
//         graph.to_svg("tests/3-2.svg").unwrap();
//         let is_balancer = Z3Backend::new(graph).is_balancer();
//         assert!(matches!(is_balancer, SatResult::Sat));
//     }

//     #[test]
//     fn balancer_3_2_broken() {
//         let entities = file_to_entities("tests/3-2-broken").unwrap();
//         let mut graph = Compiler::new(entities).create_graph();
//         graph.simplify(&[3], Aggressive);
//         graph.to_svg("tests/3-2-broken.svg").unwrap();
//         let is_balancer = Z3Backend::new(graph).is_balancer();
//         assert!(matches!(is_balancer, SatResult::Unsat));
//     }

//     #[test]
//     fn balancer_2_4_broken() {
//         let entities = file_to_entities("tests/2-4-broken").unwrap();
//         let mut graph = Compiler::new(entities).create_graph();
//         graph.simplify(&[2, 7], Aggressive);
//         graph.to_svg("tests/2-4-broken.svg").unwrap();
//         let is_balancer = Z3Backend::new(graph).is_balancer();
//         assert!(matches!(is_balancer, SatResult::Unsat));
//     }
// }
