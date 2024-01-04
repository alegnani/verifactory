mod model_entities;
mod model_graph;

pub use model_graph::Z3Backend;

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        frontend::Compiler,
        import::file_to_entities,
        ir::{CoalesceStrength::Aggressive, FlowGraphFun},
    };

    #[test]
    fn model_3_2_broken() {
        let entities = file_to_entities("tests/3-2-broken").unwrap();
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[4, 5, 6], Aggressive);
        let solver = Z3Backend::new(graph);
        solver.model();
    }

    #[test]
    fn model_4_4() {
        let entities = file_to_entities("tests/4-4").unwrap();
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[3], Aggressive);
        let solver = Z3Backend::new(graph);
        solver.model();
    }
}
