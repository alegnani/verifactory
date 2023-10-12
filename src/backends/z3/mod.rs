mod model_entities;
mod model_graph;

pub use model_graph::Z3Backend;

#[cfg(test)]
mod test {
    use super::*;
    use crate::{compiler::Compiler, ir::FlowGraphFun, utils::load_entities};

    #[test]
    fn model_3_2_broken() {
        let entities = load_entities("tests/3-2-broken");
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[4, 5, 6]);
        graph.to_svg("tests/3-2-broken.svg").unwrap();
        let solver = Z3Backend::new(graph);
        solver.model();
    }

    #[test]
    fn model_4_4() {
        let entities = load_entities("tests/4-4");
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[3]);
        graph.to_svg("tests/4-4.svg").unwrap();
        let solver = Z3Backend::new(graph);
        solver.model();
    }
}
