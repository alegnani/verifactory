use z3::{
    ast::{Ast, Bool},
    SatResult,
};

use super::Z3Backend;

pub trait Z3Proofs {
    fn is_balancer(&self) -> SatResult;
}

impl Z3Proofs for Z3Backend {
    fn is_balancer(&self) -> SatResult {
        let helper = self.model();
        let solver = self.get_solver();
        let ctx = self.get_ctx();
        let outputs = helper.output_map.values().collect::<Vec<_>>();
        let pairwise_out_eq = outputs
            .windows(2)
            .map(|w| w[0]._eq(w[1]))
            .collect::<Vec<_>>();
        let slice = pairwise_out_eq.iter().collect::<Vec<_>>();
        let out_eq = Bool::and(ctx, &slice);

        // Sanity check
        assert!(matches!(
            solver.check_assumptions(&[out_eq.clone()]),
            SatResult::Sat
        ));

        match solver.check_assumptions(&[out_eq.not()]) {
            SatResult::Unsat => SatResult::Sat,
            SatResult::Sat => SatResult::Unsat,
            SatResult::Unknown => SatResult::Unknown,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{compiler::Compiler, ir::FlowGraphFun, utils::load_entities};

    use super::*;

    #[test]
    fn balancer_3_2() {
        let entities = load_entities("tests/3-2");
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[3]);
        graph.to_svg("tests/3-2.svg").unwrap();
        let is_balancer = Z3Backend::new(graph).is_balancer();
        assert!(matches!(is_balancer, SatResult::Sat));
    }

    #[test]
    fn balancer_3_2_broken() {
        let entities = load_entities("tests/3-2-broken");
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[3]);
        graph.to_svg("tests/3-2-broken.svg").unwrap();
        let is_balancer = Z3Backend::new(graph).is_balancer();
        assert!(matches!(is_balancer, SatResult::Unsat));
    }

    #[test]
    fn balancer_2_4_broken() {
        let entities = load_entities("tests/2-4-broken");
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[2, 7]);
        graph.to_svg("tests/2-4-broken.svg").unwrap();
        let is_balancer = Z3Backend::new(graph).is_balancer();
        assert!(matches!(is_balancer, SatResult::Unsat));
    }
}
