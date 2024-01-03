use petgraph::prelude::{EdgeIndex, NodeIndex};
use std::{collections::HashMap, mem};
use z3::{
    ast::{exists_const, forall_const, Ast, Bool, Int, Real},
    Context, SatResult, Solver,
};

use crate::{
    backends::{
        proofs::Negatable,
        z3_quant::model_entities_blocked::{Z3EdgeBlocked, Z3NodeBlocked},
    },
    entities::FBEntity,
    ir::FlowGraph,
};

use super::{
    model_entities::{Z3Edge, Z3Node},
    model_entities_relaxed::{Z3EdgeRelaxed, Z3NodeRelaxed},
};

#[derive(Default)]
pub struct Z3QuantHelper<'a> {
    pub edge_map: HashMap<EdgeIndex, Real<'a>>,
    pub input_map: HashMap<NodeIndex, Int<'a>>,
    pub output_map: HashMap<NodeIndex, Real<'a>>,
    pub input_const: Vec<Bool<'a>>,
    pub others: Vec<Bool<'a>>,
    pub blocked_edge_map: HashMap<EdgeIndex, Bool<'a>>,
    pub blocked_input_map: HashMap<NodeIndex, Bool<'a>>,
    pub blocked_output_map: HashMap<NodeIndex, Bool<'a>>,
    pub blocking: Vec<Bool<'a>>,
}

pub struct ProofPrimitives<'a> {
    /// Z3 context
    pub ctx: &'a Context,
    /// Flowgraph associated with the proof
    pub graph: &'a FlowGraph,
    /// `Vec` of all the input throughput variables in z3
    pub input_bounds: Vec<Int<'a>>,
    /// Map from `NodeIndex` to the associated throughput variable in z3
    pub input_map: HashMap<NodeIndex, Int<'a>>,
    /// `Vec` of all the output throughput variables in z3
    pub output_bounds: Vec<Real<'a>>,
    /// Map from `NodeIndex` to the associated throughput variable in z3
    pub output_map: HashMap<NodeIndex, Real<'a>>,
    /// Map from `NodeIndex` to the associated input blocked variable in z3
    pub blocked_input_map: HashMap<NodeIndex, Bool<'a>>,
    /// Map from `NodeIndex` to the associated output blocked variable in z3
    pub blocked_output_map: HashMap<NodeIndex, Bool<'a>>,
    /// min. and max. throughput of an edge constraint
    pub edge_bounds: Vec<Real<'a>>,
    /// constraints like kirchhoffs law or implementation of splitters
    pub model_constraint: Bool<'a>,
    /// blocking constraints
    pub blocking_constraint: Vec<Bool<'a>>,
}

pub enum ModelType {
    Normal,
    Relaxed,
    Blocked,
}

pub fn model_f<'a, F>(
    graph: &'a FlowGraph,
    ctx: &'a Context,
    f: F,
    model_type: ModelType,
) -> SatResult
where
    F: FnOnce(ProofPrimitives<'a>) -> Bool<'a>,
{
    let solver = Solver::new(ctx);

    let mut helper = Z3QuantHelper::default();
    // encode edges as variables in z3
    for edge_idx in graph.edge_indices() {
        let edge = graph[edge_idx];
        match model_type {
            ModelType::Normal => edge.model(graph, edge_idx, ctx, &mut helper),
            ModelType::Relaxed => edge.model_relaxed(graph, edge_idx, ctx, &mut helper),
            ModelType::Blocked => edge.model_blocked(graph, edge_idx, ctx, &mut helper),
        }
    }
    // encode nodes as equations
    for node_idx in graph.node_indices() {
        let node = &graph[node_idx];
        match model_type {
            ModelType::Normal => node.model(graph, node_idx, ctx, &mut helper),
            ModelType::Relaxed => node.model_relaxed(graph, node_idx, ctx, &mut helper),
            ModelType::Blocked => node.model_blocked(graph, node_idx, ctx, &mut helper),
        }
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

    let model_constraint = vec_and(ctx, &helper.others);

    let blocking_constraint = helper.blocking;

    let primitives = ProofPrimitives {
        ctx,
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

    solver.assert(&f(primitives));
    let res = solver.check().not();
    // TODO: move to tracing
    println!("Solver:\n{:?}", solver);
    println!("Model:\n{:?}", solver.get_model());
    res
}

/// Conjunction of a slice of `Bool`s.
pub fn vec_and<'a>(ctx: &'a Context, vec: &[Bool<'a>]) -> Bool<'a> {
    let slice = vec.iter().collect::<Vec<_>>();
    Bool::and(ctx, &slice)
}

/// Equality of a slice of `Ast`s.
pub fn equality<'a, T>(ctx: &'a Context, values: &[T]) -> Bool<'a>
where
    T: Ast<'a> + Sized,
{
    let pairwise_eq = values
        .windows(2)
        .map(|w| w[0]._eq(&w[1]))
        .collect::<Vec<_>>();
    let slice = pairwise_eq.iter().collect::<Vec<_>>();
    Bool::and(ctx, &slice)
}

/// Function to prove if a given z3 model is a valid belt balancer
///
/// # Definiton
///
/// Belt balancer: Blueprint that taking every possible combination of inputs produces equal outputs.
///
/// The `balancer_condition` states that all the outputs have the same value.
/// The `model_condition` states that the z3 model is modelled correctly and that the `balancer condition` is NOT met.
/// This is used to find a counter-example.
/// Additionally the `trivial` constraint is added that constraints all inputs and outputs to be positive.
pub fn belt_balancer_f(p: ProofPrimitives<'_>) -> Bool<'_> {
    let balancer_condition = equality(p.ctx, &p.output_bounds);
    // Correct model and NOT output equality
    let model_condition = Bool::and(p.ctx, &[&balancer_condition.not(), &p.model_constraint]);

    // Model edge throughput as existentially quantified variables
    let cast_edge_bounds = p
        .edge_bounds
        .iter()
        .map(|r| r as &dyn Ast)
        .collect::<Vec<_>>();
    let ex = exists_const(p.ctx, &cast_edge_bounds, &[], &model_condition);

    // add (0 <= input) and (0 <= output) to global context
    let zero = Int::from_i64(p.ctx, 0);
    let trivial = p
        .input_bounds
        .iter()
        .map(|i| i.ge(&zero))
        .chain(p.output_bounds.iter().map(|o| o.ge(&zero.to_real())))
        .collect::<Vec<_>>();
    let trivial = vec_and(p.ctx, &trivial);

    Bool::and(p.ctx, &[&trivial, &ex])
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
/// Additionally the `trivial` constraint is added that constraints all inputs and outputs to be positive.
pub fn equal_drain_f(p: ProofPrimitives<'_>) -> Bool<'_> {
    let input_eq = equality(p.ctx, &p.input_bounds);
    let output_eq = equality(p.ctx, &p.output_bounds);
    // Correct model and equality of inputs does NOT imply equality of outputs
    let model_condition = Bool::and(
        p.ctx,
        &[&p.model_constraint, &input_eq.implies(&output_eq).not()],
    );

    // Model edge throughput as existentially quantified variables
    let cast_edge_bounds = p
        .edge_bounds
        .iter()
        .map(|r| r as &dyn Ast)
        .collect::<Vec<_>>();
    let ex = exists_const(p.ctx, &cast_edge_bounds, &[], &model_condition);

    // add (0 <= input) and (0 <= output) to global context
    let zero = Int::from_i64(p.ctx, 0);
    let trivial = p
        .input_bounds
        .iter()
        .map(|i| i.ge(&zero))
        .chain(p.output_bounds.iter().map(|o| o.ge(&zero.to_real())))
        .collect::<Vec<_>>();
    let trivial = vec_and(p.ctx, &trivial);

    Bool::and(p.ctx, &[&trivial, &ex])
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
/// TODO: fill
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
) -> impl Fn(ProofPrimitives<'a>) -> Bool<'a> {
    let i = move |p: ProofPrimitives<'a>| {
        let zero = Int::from_i64(p.ctx, 0);
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
                let upper_const = Int::from_i64(p.ctx, capacity);
                let upper = v.le(&upper_const);
                Bool::and(p.ctx, &[&lower, &upper])
            })
            .collect::<Vec<_>>();
        let input_condition = vec_and(p.ctx, &input_constraints);

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
                let upper_const = Real::from_int(&Int::from_i64(p.ctx, capacity));
                let upper = v.le(&upper_const);
                Bool::and(p.ctx, &[&lower, &upper])
            })
            .collect::<Vec<_>>();
        let output_condition = vec_and(p.ctx, &output_constraints);

        let outputs = p.output_map.values().collect::<Vec<_>>();
        let output_sum = Real::add(p.ctx, &outputs);

        let inputs = p.input_map.values().collect::<Vec<_>>();
        let input_sum = Real::from_int(&Int::add(p.ctx, &inputs));

        let in_out_eq = input_sum._eq(&output_sum);

        // Model edge throughput as existentially quantified variables
        let cast_edge_bounds = p
            .edge_bounds
            .iter()
            .map(|r| r as &dyn Ast)
            .collect::<Vec<_>>();

        let no_model = forall_const(p.ctx, &cast_edge_bounds, &[], &p.model_constraint.not());

        Bool::and(
            p.ctx,
            &[&input_condition, &output_condition, &in_out_eq, &no_model],
        )
    };
    i
}

#[cfg(test)]
mod tests {
    use z3::Config;

    use super::*;
    use crate::backends::Printable;
    use crate::{frontend::Compiler, import::file_to_entities, ir::FlowGraphFun};

    #[test]
    fn is_throughput_unlimited() {
        let entities = file_to_entities("tests/4-4-tu").unwrap();
        let mut graph = Compiler::new(entities.clone()).create_graph();
        graph.simplify(&[], crate::ir::CoalesceStrength::Lossless);
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let res = model_f(
            &graph,
            &ctx,
            throughput_unlimited(entities),
            ModelType::Relaxed,
        );
        println!("Result: {}", res.to_str());
        assert!(matches!(res, SatResult::Sat));
    }

    #[test]
    fn not_throughput_unlimited() {
        let entities = file_to_entities("tests/4-4-ntu").unwrap();
        let mut graph = Compiler::new(entities.clone()).create_graph();
        graph.simplify(&[], crate::ir::CoalesceStrength::Lossless);
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let res = model_f(
            &graph,
            &ctx,
            throughput_unlimited(entities),
            ModelType::Relaxed,
        );
        println!("Result: {}", res.to_str());
        assert!(matches!(res, SatResult::Unsat));
    }
}
