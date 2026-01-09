use crate::Prediction;
use crate::trace::dag::{Graph, NodeType};
use std::sync::{Arc, Mutex};
use tokio::task_local;

task_local! {
    static CURRENT_TRACE: Arc<Mutex<Graph>>;
}

pub async fn trace<F, Fut, R>(f: F) -> (R, Graph)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = R>,
{
    let graph = Arc::new(Mutex::new(Graph::new()));
    let result = CURRENT_TRACE.scope(graph.clone(), f()).await;

    // We need to unwrap the graph.
    // If there are other references (which shouldn't be if scope ended and we are the only owner of the Arc),
    // try_unwrap works.
    // However, if tasks are still running (orphaned), this might fail.
    // Assuming well-behaved usage.
    let graph = match Arc::try_unwrap(graph) {
        Ok(mutex) => mutex.into_inner().unwrap(),
        Err(arc) => arc.lock().unwrap().clone(), // Fallback: clone if still shared
    };

    (result, graph)
}

pub fn is_tracing() -> bool {
    CURRENT_TRACE.try_with(|_| ()).is_ok()
}

pub fn record_node(
    node_type: NodeType,
    inputs: Vec<usize>,
    input_data: Option<crate::Example>,
) -> Option<usize> {
    CURRENT_TRACE
        .try_with(|trace| {
            let mut graph = trace.lock().unwrap();
            Some(graph.add_node(node_type, inputs, input_data))
        })
        .unwrap_or(None)
}

pub fn record_output(node_id: usize, output: Prediction) {
    let _ = CURRENT_TRACE.try_with(|trace| {
        let mut graph = trace.lock().unwrap();
        graph.set_output(node_id, output);
    });
}
