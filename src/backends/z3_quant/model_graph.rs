use petgraph::prelude::{EdgeIndex, NodeIndex};
use std::{collections::HashMap, mem};
use z3::{
    ast::{exists_const, forall_const, Ast, Bool, Int, Real},
    Context, SatResult, Solver,
};

use crate::{backends::proofs::Negatable, entities::Entity, ir::FlowGraph};

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
    pub ctx: &'a Context,
    pub graph: &'a FlowGraph,
    pub input_bounds: Vec<Int<'a>>,
    pub input_map: HashMap<NodeIndex, Int<'a>>,
    pub output_bounds: Vec<Real<'a>>,
    pub edge_bounds: Vec<Real<'a>>,
    pub model_constraint: Bool<'a>,
}

pub fn model_f<'a, F>(graph: &'a FlowGraph, ctx: &'a Context, f: F) -> SatResult
where
    F: FnOnce(ProofPrimitives<'a>) -> Bool<'a>,
{
    // let cfg = Config::new();
    // let ctx = Context::new(&cfg);
    let solver = Solver::new(ctx);

    let mut helper = Z3QuantHelper::default();
    /* encode edges as variables in z3 */
    for edge_idx in graph.edge_indices() {
        let edge = graph[edge_idx];
        edge.model(edge_idx, ctx, &mut helper);
    }
    /* encode nodes as equations */
    for node_idx in graph.node_indices() {
        let node = &graph[node_idx];
        node.model(graph, node_idx, ctx, &mut helper);
    }

    /* add stuff to solver */
    let input_map = mem::take(&mut helper.input_map);
    let input_bounds = input_map
        .values()
        .cloned()
        // .map(|i| i as &dyn Ast)
        .collect::<Vec<_>>();

    let output_map = mem::take(&mut helper.output_map);
    let output_bounds = output_map
        .values()
        .cloned()
        // .map(|o| o as &dyn Ast)
        .collect::<Vec<_>>();

    let edge_map = mem::take(&mut helper.edge_map);
    let edge_bounds = edge_map
        .values()
        .cloned()
        // .map(|e| e as &dyn Ast)
        .collect::<Vec<_>>();

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
    println!("{:?}", solver);
    println!("{:?}", solver.get_model());
    res
}

fn vec_and<'a>(ctx: &'a Context, vec: &[Bool<'a>]) -> Bool<'a> {
    let slice = vec.iter().collect::<Vec<_>>();
    Bool::and(ctx, &slice)
}

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

pub fn belt_balancer_f(p: ProofPrimitives<'_>) -> Bool<'_> {
    let balancer_condition = equality(p.ctx, &p.output_bounds);
    let model_condition = Bool::and(p.ctx, &[&balancer_condition.not(), &p.model_constraint]);
    let cast_edge_bounds = p
        .edge_bounds
        .iter()
        .map(|r| r as &dyn Ast)
        .collect::<Vec<_>>();
    let ex = exists_const(p.ctx, &cast_edge_bounds, &[], &model_condition);

    /* add (0 <= input) and (0 <= output) to global context s.t. it appears in model */
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

pub fn equal_drain_f(p: ProofPrimitives<'_>) -> Bool<'_> {
    let input_eq = equality(p.ctx, &p.input_bounds);
    let output_eq = equality(p.ctx, &p.output_bounds);
    let model_condition = Bool::and(
        p.ctx,
        &[&p.model_constraint, &input_eq.implies(&output_eq).not()],
    );
    let cast_edge_bounds = p
        .edge_bounds
        .iter()
        .map(|r| r as &dyn Ast)
        .collect::<Vec<_>>();
    let ex = exists_const(p.ctx, &cast_edge_bounds, &[], &model_condition);

    /* add (0 <= input) and (0 <= output) to global context s.t. it appears in model */
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

pub fn throughput_unlimited<'a>(
    entities: Vec<Entity<i32>>,
) -> impl Fn(ProofPrimitives<'a>) -> Bool<'a> {
    let i = move |p: ProofPrimitives<'a>| {
        let zero = Int::from_i64(p.ctx, 0);
        let input_cond = p
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

        let input = vec_and(p.ctx, &input_cond);
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
        Bool::and(p.ctx, &[&input, &all_outputs])
    };
    i
}
