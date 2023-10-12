use z3::{
    ast::{Ast, Bool},
    SatResult,
};

use super::Z3Backend;

pub trait Z3Proofs {
    fn is_balancer(&self) -> SatResult;
    fn is_equal_drain_balancer(&self) -> SatResult;
}

trait Negatable {
    fn not(self) -> Self;
}

impl Negatable for SatResult {
    fn not(self) -> Self {
        match self {
            SatResult::Sat => SatResult::Unsat,
            SatResult::Unsat => SatResult::Sat,
            SatResult::Unknown => SatResult::Unknown,
        }
    }
}

impl Z3Backend {
    fn equality<'a, T>(&'a self, values: &[&'a T]) -> Bool<'_>
    where
        T: Ast<'a> + Sized,
    {
        let ctx = self.get_ctx();
        let pairwise_eq = values
            .windows(2)
            .map(|w| w[0]._eq(w[1]))
            .collect::<Vec<_>>();
        let slice = pairwise_eq.iter().collect::<Vec<_>>();
        Bool::and(ctx, &slice)
    }
}

impl Z3Proofs for Z3Backend {
    fn is_balancer(&self) -> SatResult {
        let helper = self.model();
        let solver = self.get_solver();
        let outputs = helper.output_map.values().collect::<Vec<_>>();
        let out_eq = self.equality(&outputs);

        // Sanity check
        assert!(matches!(
            solver.check_assumptions(&[out_eq.clone()]),
            SatResult::Sat
        ));

        let res = solver.check_assumptions(&[out_eq.not()]);
        res.not()
    }

    /* FIXME: this does not work due to the way mergers are modelled */
    fn is_equal_drain_balancer(&self) -> SatResult {
        match self.is_balancer() {
            SatResult::Sat => {
                let helper = self.model();
                let solver = self.get_solver();
                let outputs = helper.output_map.values().collect::<Vec<_>>();
                let inputs = helper.input_map.values().collect::<Vec<_>>();
                let out_eq = self.equality(&outputs);
                let in_eq = self.equality(&inputs);

                /* An equal drain balancer is a balancer s.t. the following holds:
                 * (out1 = out2 = ...) -> (in1 = in2 = ...) */

                let implic = out_eq.implies(&in_eq);
                // Sanity check
                assert!(matches!(
                    solver.check_assumptions(&[implic.clone()]),
                    SatResult::Sat
                ));

                let res = solver.check_assumptions(&[implic.not()]);
                res.not()
            }
            x => x,
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
