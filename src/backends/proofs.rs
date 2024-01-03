use z3::SatResult;

pub trait Z3Proofs {
    fn is_balancer(&self) -> SatResult;
    fn is_equal_drain_balancer(&self) -> SatResult;
    fn get_counter_example(&self);
}

pub trait Negatable {
    fn not(self) -> Self;
}

pub trait Printable {
    fn to_str(&self) -> &'static str;
}

impl Printable for SatResult {
    fn to_str(&self) -> &'static str {
        match self {
            Self::Sat => "Yes",
            Self::Unsat => "No",
            Self::Unknown => "Unknown",
        }
    }
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
