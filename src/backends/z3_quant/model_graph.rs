use petgraph::prelude::{EdgeIndex, NodeIndex};
use std::{collections::HashMap, mem};
use z3::{
    ast::{exists_const, forall_const, Ast, Bool, Int, Real},
    Context, SatResult, Solver,
};

use crate::{backends::proofs::Negatable, entities::FBEntity, ir::FlowGraph};

use super::model_entities::{Z3Edge, Z3Node};

#[derive(Default)]
pub struct Z3QuantHelper<'a> {
    pub edge_map: HashMap<EdgeIndex, Real<'a>>,
    pub input_map: HashMap<NodeIndex, Int<'a>>,
    pub output_map: HashMap<NodeIndex, Real<'a>>,
    pub input_const: Vec<Bool<'a>>,
    pub others: Vec<Bool<'a>>,
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
    /// min. and max. throughput of an edge constraint
    pub edge_bounds: Vec<Real<'a>>,
    /// constraints like kirchhoffs law or implementation of splitters
    pub model_constraint: Bool<'a>,
}

pub fn model_f<'a, F>(graph: &'a FlowGraph, ctx: &'a Context, f: F) -> SatResult
where
    F: FnOnce(ProofPrimitives<'a>) -> Bool<'a>,
{
    let solver = Solver::new(ctx);

    let mut helper = Z3QuantHelper::default();
    // encode edges as variables in z3
    for edge_idx in graph.edge_indices() {
        let edge = graph[edge_idx];
        edge.model(edge_idx, ctx, &mut helper);
    }
    // encode nodes as equations
    for node_idx in graph.node_indices() {
        let node = &graph[node_idx];
        node.model(graph, node_idx, ctx, &mut helper);
    }

    // add stuff to solver
    let input_map = mem::take(&mut helper.input_map);
    let input_bounds = input_map.values().cloned().collect::<Vec<_>>();

    let output_map = mem::take(&mut helper.output_map);
    let output_bounds = output_map.values().cloned().collect::<Vec<_>>();

    let edge_map = mem::take(&mut helper.edge_map);
    let edge_bounds = edge_map.values().cloned().collect::<Vec<_>>();

    let model_constraint = vec_and(ctx, &helper.others);

    let primitives = ProofPrimitives {
        ctx,
        graph,
        input_bounds,
        input_map,
        output_bounds,
        edge_bounds,
        model_constraint,
    };

    solver.assert(&f(primitives));
    let res = solver.check().not();
    // TODO: move to tracing
    println!("{:?}", solver);
    println!("{:?}", solver.get_model());
    res
}

/// Conjunction of a slice of `Bool`s.
fn vec_and<'a>(ctx: &'a Context, vec: &[Bool<'a>]) -> Bool<'a> {
    let slice = vec.iter().collect::<Vec<_>>();
    Bool::and(ctx, &slice)
}

/// Equality of a slice of `Ast`s.
fn equality<'a, T>(ctx: &'a Context, values: &[T]) -> Bool<'a>
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
/// TODO: add links, 2-2 splitter priority + bottleneck on output
/// E.g. [this]() is a 2-2 equal drain belt balancer; [this]() is only a 2-2 belt balancer.
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

        let all_edges = forall_const(
            p.ctx,
            &p.edge_bounds
                .iter()
                .map(|e| e as &dyn Ast)
                .collect::<Vec<_>>(),
            &[],
            &p.model_constraint.not(),
        );
        let all_outputs = forall_const(
            p.ctx,
            &p.output_bounds
                .iter()
                .map(|e| e as &dyn Ast)
                .collect::<Vec<_>>(),
            &[],
            &all_edges,
        );
        Bool::and(p.ctx, &[&input_condition, &all_outputs])
    };
    i
}
