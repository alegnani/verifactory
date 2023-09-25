use z3::{ast::Real, Config, Context, Optimize};

use crate::ir::{Edge, FlowGraph, Node};

type Z3InnerGraph<'a> = petgraph::Graph<Z3Node<'a>, Z3Edge<'a>, petgraph::Directed>;

struct Z3Graph {
    graph: Z3InnerGraph<'static>,
    solver: Optimize<'static>,
}

impl Z3Graph {
    /* FIXME: this creates a memory leak.
     * non-halal stuff to keep the borrow-checker happy :/ */
    pub fn from_graph(graph: FlowGraph) -> Self {
        let config = Config::new();
        let context = Box::new(Context::new(&config));
        let context = Box::leak(context);
        let solver = Optimize::new(context);

        let graph = Self::model_graph(graph, &solver);

        Self { graph, solver }
    }

    fn get_ctx(&self) -> &Context {
        self.solver.get_context()
    }

    fn get_solver(&self) -> &Optimize {
        &self.solver
    }

    fn model_graph(graph: FlowGraph, solver: &Optimize) -> Z3InnerGraph<'static> {
        todo!()
    }
}

struct Z3Node<'a> {
    node: Node,
    z3_var: Option<Real<'a>>,
}

impl<'a> Z3Node<'a> {
    fn from_node(node: Node, z3: Z3Graph) -> Self {
        /* TODO: model */
        Self { node, z3_var: None }
    }
}

trait ModelInZ3 {
    fn model_in_z3(&self, z3: &Z3Graph) -> Z3Node;
}

struct Z3Edge<'a> {
    edge: Edge,
    z3_var: Real<'a>,
}

impl<'a> Z3Edge<'a> {
    fn from_edge(id: i32, edge: Edge, z3: &Z3Graph) -> Self {
        let ctx = z3.get_ctx();

        let capacity = Real::from_real(ctx, edge.capacity as i32, 1);
        let zero = Real::from_real(ctx, 0, 1);
        let edge_name = format!("edge{}", id);
        let edge_var = Real::new_const(ctx, edge_name);

        z3.solver.assert(&edge_var.le(&capacity));
        z3.solver.assert(&edge_var.le(&zero));

        Self {
            edge,
            z3_var: edge_var,
        }
    }
}
