use verifactory_lib::{
    backends::{belt_balancer_f, equal_drain_f, model_f, ModelFlags, ProofResult},
    frontend::Compiler,
    import::file_to_entities,
    ir::{CoalesceStrength, FlowGraphFun, Reversable},
};
use z3::{Config, Context};

#[test]
fn reverse_equivalence() {
    // Tests for issue #20
    let entities = file_to_entities("blueprints/2-5").unwrap();
    let mut graph = Compiler::new(entities).create_graph();
    graph.reverse();
    graph.simplify(&[17, 18, 19], CoalesceStrength::Aggressive);
    let cfg = Config::new();
    let ctx25 = Context::new(&cfg);
    let res25 = model_f(&graph, &ctx25, equal_drain_f, ModelFlags::empty());
    println!("2-5 equal drain: {}", res25);

    let entities = file_to_entities("blueprints/5-2").unwrap();
    let mut graph = Compiler::new(entities).create_graph();
    graph.simplify(&[19, 22, 25], CoalesceStrength::Aggressive);
    let ctx52 = Context::new(&cfg);
    let res52 = model_f(&graph, &ctx52, belt_balancer_f, ModelFlags::empty());
    println!("5-2 belt balancer: {}", res52);

    assert_eq!(res25, res52);
}
