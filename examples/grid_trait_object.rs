use std::fmt::Debug;
use std::default::Default;

trait Graph {
    type N: Debug;
    type E;

    fn has_edge(&self, &Self::N, &Self::N) -> bool;
    fn edges(&self, &Self::N) -> Vec<Self::E>;
}

#[derive(Debug)]
struct Node;

impl Default for Node {
    fn default() -> Node {
        Node
    }
}

struct Edge;

struct MyGraph;

impl Graph for MyGraph {
    type N = Node;
    type E = Edge;

    fn has_edge(&self, _: &Node, _: &Node) -> bool {
        true
    }

    fn edges(&self, _: &Node) -> Vec<Edge> {
        Vec::new()
    }
}

fn main() {
    let graph = MyGraph;
    let obj = Box::new(graph) as Box<Graph<N = Node, E = Edge>>;
    println!("{:?}", get_edges(obj));
}

fn get_edges<N, E>(graph: Box<Graph<N = N, E = E>>) -> bool
    where N: Default + Debug
{
    let edges = graph.edges(&N::default());
    edges.len() > 5
}
