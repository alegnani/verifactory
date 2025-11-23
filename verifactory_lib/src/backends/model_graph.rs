use bitflags::bitflags;
use petgraph::prelude::{EdgeIndex, NodeIndex};
use std::{collections::HashMap, mem};
use z3::{
    ast::{exists_const, forall_const, Ast, Bool, Int, Real},
    Solver,
};

use crate::{entities::FBEntity, ir::FlowGraph};

use super::proofs::ProofResult;

use super::model_entities::{Z3Edge, Z3Node};

#[derive(Default)]
pub struct Z3QuantHelper {
    pub edge_map: HashMap<EdgeIndex, Real>,
    pub input_map: HashMap<NodeIndex, Int>,
    pub output_map: HashMap<NodeIndex, Real>,
    pub input_const: Vec<Bool>,
    pub others: Vec<Bool>,
    pub blocked_edge_map: HashMap<EdgeIndex, Bool>,
    pub blocked_input_map: HashMap<NodeIndex, Bool>,
    pub blocked_output_map: HashMap<NodeIndex, Bool>,
    pub blocking: Vec<Bool>,
}

#[derive(Debug, Clone)]
pub struct ProofPrimitives<'a> {
    /// Flowgraph associated with the proof
    pub graph: &'a FlowGraph,
    /// `Vec` of all the input throughput variables in z3
    pub input_bounds: Vec<Int>,
    /// Map from `NodeIndex` to the associated throughput variable in z3
    pub input_map: HashMap<NodeIndex, Int>,
    /// `Vec` of all the output throughput variables in z3
    pub output_bounds: Vec<Real>,
    /// Map from `NodeIndex` to the associated throughput variable in z3
    pub output_map: HashMap<NodeIndex, Real>,
    /// Map from `NodeIndex` to the associated input blocked variable in z3
    pub blocked_input_map: HashMap<NodeIndex, Bool>,
    /// Map from `NodeIndex` to the associated output blocked variable in z3
    pub blocked_output_map: HashMap<NodeIndex, Bool>,
    /// min. and max. throughput of an edge constraint
    pub edge_bounds: Vec<Real>,
    /// constraints like kirchhoffs law or implementation of splitters
    pub model_constraint: Bool,
    /// blocking constraints
    pub blocking_constraint: Vec<Bool>,
}

bitflags! {
    #[derive(Clone, Copy)]
    pub struct ModelFlags: u8 {
        const Relaxed = 1;
        const Blocked = 1 << 1;
    }
}

pub fn model_f<'a, F>(graph: &'a FlowGraph, f: F, flags: ModelFlags) -> ProofResult
where
    F: FnOnce(ProofPrimitives<'a>) -> Bool,
{
    let solver = Solver::new();

    let mut helper = Z3QuantHelper::default();
    // encode edges as variables in z3
    for edge_idx in graph.edge_indices() {
        let edge = graph[edge_idx];
        edge.model(graph, edge_idx, &mut helper, flags);
    }
    // encode nodes as equations
    for node_idx in graph.node_indices() {
        let node = &graph[node_idx];
        node.model(graph, node_idx, &mut helper, flags);
    }

    // add stuff to solver
    let input_map = mem::take(&mut helper.input_map);
    let input_bounds = input_map.values().cloned().collect::<Vec<_>>();

    let output_map = mem::take(&mut helper.output_map);
    let output_bounds = output_map.values().cloned().collect::<Vec<_>>();

    let blocked_input_map = mem::take(&mut helper.blocked_input_map);
    let blocked_output_map = mem::take(&mut helper.blocked_output_map);

    let edge_map = mem::take(&mut helper.edge_map);
    let edge_bounds = edge_map.values().cloned().collect::<Vec<_>>();

    let model_constraint = vec_and(&helper.others);

    let blocking_constraint = helper.blocking;

    let primitives = ProofPrimitives {
        graph,
        input_bounds,
        input_map,
        output_bounds,
        output_map,
        blocked_input_map,
        blocked_output_map,
        edge_bounds,
        model_constraint,
        blocking_constraint,
    };

    solver.assert(&f(primitives.clone()));
    let res: ProofResult = solver.check().into();
    // TODO: move to tracing
    // println!("Solver:\n{:?}", solver);
    // println!("Model:\n{:?}", solver.get_model());
    if let Some(model) = solver.get_model() {
        for input in primitives.input_bounds {
            let a = model.eval(&input, true);
            println!("{:?}: {:?}", &input, a);
        }
    }
    res.not()
}

/// Conjunction of a slice of `Bool`s.
pub fn vec_and<'a>(vec: &[Bool]) -> Bool {
    let slice = vec.iter().collect::<Vec<_>>();
    Bool::and(&slice)
}

/// Equality of a slice of `Ast`s.
pub fn equality<'a, T>(values: &'a [T]) -> Bool
where
    T: Ast + Sized + std::convert::From<&'a T> + 'a,
{
    let pairwise_eq = values
        .windows(2)
        .map(|w| w[0].eq(&w[1]))
        .collect::<Vec<_>>();
    let slice = pairwise_eq.iter().collect::<Vec<_>>();
    Bool::and(&slice)
}

/// Function to prove if a given z3 model is a valid belt balancer
///
/// # Definiton
///
/// Belt balancer: Blueprint that taking every possible combination of inputs produces equal outputs.
///
/// The `balancer_condition` states that all the outputs have the same value.
/// Finding values s.t. the model is satisfied and output equality is not achieve, constitues a counter-example.
pub fn belt_balancer_f(p: ProofPrimitives<'_>) -> Bool {
    let balancer_condition = equality(&p.output_bounds);
    // Correct model and NOT output equality
    Bool::and(&[&balancer_condition.not(), &p.model_constraint])
}

/// Function to prove if a given z3 model is an equal drain belt balancer
///
/// # Definiton
///
/// Equal drain: When operating all the inputs are consumed equally, not resulting in any imbalances.
/// E.g. [this](https://fbe.teoxoy.com/?source=0eJyd0ttqwzAMBuB30bVTkjSliy/3GqUUJ1WLwFGMrYyFkHefnYxRGPR0aVv/JyN7gsYO6DyxgJ6A2p4D6MMEga5sbNqT0SFoIMEOFLDp0upigmTiDQfXe8katAKzAuIzfoMu5qMCZCEhXLnbWHCWRNBHzfUh1vSc+sRcrmAEneWbXbTO5LFdD/NfbTzx0DUxGTuo6d5d/tEJXfXisV6+qr+Cb9/FnxhL9fZYnsB36VWXb6Bvfo2CL/RhiZQfRbWvy31V1vW2LhRYE7vG6s+/6nn+AWY2ztQ=) is a 2-2 equal drain belt balancer;
/// [this](https://fbe.teoxoy.com/?source=0eJyVktFqwzAMRf9Fz/ZI0pQuftxvlFGcVh0GRza2MhZC/n1yOkbHYGuejC3dc7myZuj9iDE5YjAzuHOgDOY4Q3ZvZH154ykiGHCMAyggO5Tb1WbWnCzlGBLrHj3DosDRBT/A1MurAiR27PCG+0OmIIYsnYGKm6h19bRXMIGRU5gXl/B8K1df1OlE49BjKk7qJzxH75il9AtbrUz9ALQpQeLIJ5lLSFIRuMdrSbgtyYYguzv2wwPS9f/gdgN4C3df/nhdCnO3QwreMeVV0jzX7aFrDm3TdbuuVuCtuEr3y3f3snwCz/TTyA==) is only a 2-2 belt balancer.
///
/// # Precondition
///
/// Assumes that the model is a valid belt balancer.
/// Uses a reversed graph.
///
/// The `model_condition` states that the z3 model is modelled correctly and that equality of inputs does NOT imply equality of outputs.
/// This is used to find a counter-example.
pub fn equal_drain_f(p: ProofPrimitives<'_>) -> Bool {
    let input_eq = equality(&p.input_bounds);
    let output_eq = equality(&p.output_bounds);
    // Correct model and equality of inputs does NOT imply equality of outputs
    Bool::and(&[&p.model_constraint, &input_eq.implies(&output_eq).not()])
}

// TODO: figure out lifetimes and fix code duplication
fn capacity_bound<'a, 'b>(
    p: &'a ProofPrimitives<'a>,
    entities: &[FBEntity<i32>],
    iter: impl Iterator<Item = (&'b NodeIndex, &'a Real)>,
) -> Bool {
    let zero = Real::from_rational(0, 1);
    let conditions = iter
        .map(|(idx, v)| {
            let lower = v.ge(&zero);

            let entity_id = p.graph[*idx].get_id();
            let capacity = entities
                .iter()
                .find(|e| e.get_base().id == entity_id)
                .unwrap()
                .get_base()
                .throughput as i64;
            let upper_const = Real::from_int(&Int::from_i64(capacity));
            let upper = v.le(&upper_const);
            Bool::and(&[&lower, &upper])
        })
        .collect::<Vec<_>>();
    vec_and(&conditions)
}

/// Function that generates a function to prove if a given z3 model is a throughput unlimited belt balancer
///
/// # Definition
///
/// Throughput unlimited:
///
/// # Precondition
///
/// Assumes that the model is a valid belt balancer.
///
/// To prove:
/// ```text
/// forall inputs, outputs. in_out_eq -> exist edges. model holds
/// ```
/// Find a counterexample:
/// ```text
/// not forall inputs, outputs. in_out_eq -> exist edges. model holds
/// not forall inputs, outputs. not in_out_eq or exist edges. model holds
/// exist inputs, outputs. in_out_eq and not exist edges. model holds
/// inputs, outputs. in_out_eq and forall edges. model does NOT hold
/// ```
pub fn throughput_unlimited<'a>(
    entities: Vec<FBEntity<i32>>,
) -> impl Fn(ProofPrimitives<'a>) -> Bool {
    let i = move |p: ProofPrimitives<'a>| {
        let zero = Int::from_i64(0);
        // `input_condition` adds the following constraint to all inputs (0 <= input <= capacity)
        let input_constraints = p
            .input_map
            .iter()
            .map(|(idx, v)| {
                let lower = v.ge(&zero);

                let entity_id = p.graph[*idx].get_id();
                let capacity = entities
                    .iter()
                    .find(|e| e.get_base().id == entity_id)
                    .unwrap()
                    .get_base()
                    .throughput as i64;
                let upper_const = Int::from_i64(capacity);
                let upper = v.le(&upper_const);
                Bool::and(&[&lower, &upper])
            })
            .collect::<Vec<_>>();
        let input_condition = vec_and(&input_constraints);

        let zero = Real::from_int(&zero);
        // `output_condition` adds the following constraint to all outputs (0 <= output <= capacity)
        let output_constraints = p
            .output_map
            .iter()
            .map(|(idx, v)| {
                let lower = v.ge(&zero);

                let entity_id = p.graph[*idx].get_id();
                let capacity = entities
                    .iter()
                    .find(|e| e.get_base().id == entity_id)
                    .unwrap()
                    .get_base()
                    .throughput as i64;
                let upper_const = Real::from_int(&Int::from_i64(capacity));
                let upper = v.le(&upper_const);
                Bool::and(&[&lower, &upper])
            })
            .collect::<Vec<_>>();
        let output_condition = vec_and(&output_constraints);

        let outputs = p.output_map.values().collect::<Vec<_>>();
        let output_sum = if !outputs.is_empty() {
            Real::add(&outputs)
        } else {
            zero.clone()
        };

        let inputs = p.input_map.values().collect::<Vec<_>>();
        let input_sum = if !inputs.is_empty() {
            Real::from_int(&Int::add(&inputs))
        } else {
            zero
        };

        let in_out_eq = input_sum.eq(&output_sum);

        // Model edge throughput as existentially quantified variables
        let cast_edge_bounds = p
            .edge_bounds
            .iter()
            .map(|r| r as &dyn Ast)
            .collect::<Vec<_>>();

        let no_model = forall_const(&cast_edge_bounds, &[], &p.model_constraint.not());

        Bool::and(&[&input_condition, &output_condition, &in_out_eq, &no_model])
    };
    i
}

/// input, output, blocked. BLOCKING, MODEL and not OUT_EQ
pub fn universal_balancer(p: ProofPrimitives<'_>) -> Bool {
    let eq_value = Real::new_const("output_value");
    let outputs_eq_value = p
        .output_map
        .iter()
        .map(|(idx, output)| {
            let is_blocked = p.blocked_output_map.get(idx).unwrap();
            is_blocked.not().implies(&output.eq(&eq_value))
        })
        .collect::<Vec<_>>();
    let out_eq = vec_and(&outputs_eq_value);
    let out_eq_condition = exists_const(&[&eq_value], &[], &out_eq);
    let blocking_p = vec_and(&p.blocking_constraint);
    Bool::and(&[&blocking_p, &p.model_constraint, &out_eq_condition.not()])
}

#[cfg(test)]
mod tests {
    use z3::Config;

    use super::*;
    use crate::ir::CoalesceStrength;
    use crate::{frontend::Compiler, import::file_to_entities, ir::FlowGraphFun};

    // TODO: figure out lifetimes and fix code duplication
    #[test]
    fn is_balancer_3_2_broken() {
        let entities = file_to_entities("tests/3-2-broken").unwrap();
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[4, 5, 6], CoalesceStrength::Aggressive);
        let res = model_f(&graph, belt_balancer_f, ModelFlags::empty());
        println!("Result: {}", res);
        assert!(matches!(res, ProofResult::Unsat));
    }

    #[test]
    fn is_balancer_4_4() {
        let entities = file_to_entities("tests/4-4").unwrap();
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[3], CoalesceStrength::Aggressive);
        let res = model_f(&graph, belt_balancer_f, ModelFlags::empty());
        println!("Result: {}", res);
        assert!(matches!(res, ProofResult::Sat));
    }

    #[test]
    fn is_throughput_unlimited_4_4() {
        let entities = file_to_entities("tests/4-4-tu").unwrap();
        let mut graph = Compiler::new(entities.clone()).create_graph();
        graph.simplify(&[], CoalesceStrength::Aggressive);
        let res = model_f(&graph, throughput_unlimited(entities), ModelFlags::Relaxed);
        println!("Result: {}", res);
        assert!(matches!(res, ProofResult::Sat));
    }

    #[test]
    fn not_throughput_unlimited_4_4() {
        let entities = file_to_entities("tests/4-4-ntu").unwrap();
        let mut graph = Compiler::new(entities.clone()).create_graph();
        graph.simplify(&[], CoalesceStrength::Aggressive);
        let res = model_f(&graph, throughput_unlimited(entities), ModelFlags::Relaxed);
        println!("Result: {}", res);
        assert!(matches!(res, ProofResult::Unsat));
    }

    #[test]
    fn is_throughput_unlimited_6_3() {
        let entities = file_to_entities("tests/6-3-tu").unwrap();
        let mut graph = Compiler::new(entities.clone()).create_graph();
        graph.simplify(&[24, 36, 44], CoalesceStrength::Aggressive);
        let res = model_f(&graph, throughput_unlimited(entities), ModelFlags::Relaxed);
        println!("Result: {}", res);
        assert!(matches!(res, ProofResult::Sat));
    }

    #[test]
    fn not_throughput_unlimited_6_3() {
        let entities = file_to_entities("tests/6-3-ntu").unwrap();
        let mut graph = Compiler::new(entities.clone()).create_graph();
        graph.simplify(&[25, 26], CoalesceStrength::Aggressive);
        let res = model_f(&graph, throughput_unlimited(entities), ModelFlags::Relaxed);
        println!("Result: {}", res);
        assert!(matches!(res, ProofResult::Unsat));
    }

    #[test]
    fn is_universal_4_4_univ() {
        let entities = file_to_entities("tests/4-4-univ").unwrap();
        let mut graph = Compiler::new(entities.clone()).create_graph();
        graph.simplify(
            &[30, 33, 83, 55, 17, 46, 133, 71],
            CoalesceStrength::Aggressive,
        );
        let res = model_f(&graph, universal_balancer, ModelFlags::Blocked);
        println!("Result: {}", res);
        assert!(matches!(res, ProofResult::Sat));
    }

    #[test]
    fn not_universal_4_4() {
        let entities = file_to_entities("tests/4-4-tu").unwrap();
        let mut graph = Compiler::new(entities.clone()).create_graph();
        graph.simplify(&[], CoalesceStrength::Aggressive);
        let res = model_f(&graph, universal_balancer, ModelFlags::Blocked);
        println!("Result: {}", res);
        assert!(matches!(res, ProofResult::Unsat));
    }

    #[test]
    fn empty_belt_balancer() {
        let entities = vec![];
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[], CoalesceStrength::Aggressive);
        let res = model_f(&graph, belt_balancer_f, ModelFlags::empty());
        assert!(matches!(res, ProofResult::Sat));
    }

    #[test]
    fn empty_equal_drain() {
        let entities = vec![];
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[], CoalesceStrength::Aggressive);
        let res = model_f(&graph, equal_drain_f, ModelFlags::empty());
        assert!(matches!(res, ProofResult::Sat));
    }

    #[test]
    fn empty_throughput_unlimited() {
        let entities = vec![];
        let mut graph = Compiler::new(entities.clone()).create_graph();
        graph.simplify(&[], CoalesceStrength::Aggressive);
        let res = model_f(&graph, throughput_unlimited(entities), ModelFlags::Relaxed);
        assert!(matches!(res, ProofResult::Sat));
    }

    #[test]
    fn empty_universal_balancer() {
        let entities = vec![];
        let mut graph = Compiler::new(entities).create_graph();
        graph.simplify(&[], CoalesceStrength::Aggressive);
        let res = model_f(&graph, equal_drain_f, ModelFlags::Blocked);
        assert!(matches!(res, ProofResult::Sat));
    }
}
