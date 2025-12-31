//! Performance benchmarks for GitAfter stacks operations

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use gitafter::stacks::graph::StackGraph;
use nostr::Event;

/// Create a test PR event for benchmarking
fn create_test_pr(
    event_id: &str,
    stack_id: &str,
    layer: u32,
    total: u32,
    depends_on: Option<&str>,
) -> Event {
    let mut tags = vec![
        vec!["stack".to_string(), stack_id.to_string()],
        vec!["layer".to_string(), layer.to_string(), total.to_string()],
    ];

    if let Some(dep) = depends_on {
        tags.push(vec!["depends_on".to_string(), dep.to_string()]);
    }

    Event {
        id: event_id.to_string(),
        kind: 1618,
        pubkey: "test_pubkey".to_string(),
        created_at: 0,
        content: String::new(),
        tags,
        sig: String::new(),
    }
}

/// Generate a linear stack (1→2→3→...→N)
fn generate_linear_stack(size: usize) -> Vec<Event> {
    let mut prs = Vec::with_capacity(size);

    for i in 0..size {
        let event_id = format!("pr{}", i);
        let depends_on = if i == 0 {
            None
        } else {
            Some(format!("pr{}", i - 1))
        };

        prs.push(create_test_pr(
            &event_id,
            "stack1",
            (i + 1) as u32,
            size as u32,
            depends_on.as_deref(),
        ));
    }

    prs
}

/// Generate a fan-out stack (1→{2,3,4,...})
fn generate_fanout_stack(size: usize) -> Vec<Event> {
    let mut prs = Vec::with_capacity(size);

    // Base layer
    prs.push(create_test_pr("pr0", "stack1", 1, size as u32, None));

    // All other layers depend on base
    for i in 1..size {
        let event_id = format!("pr{}", i);
        prs.push(create_test_pr(
            &event_id,
            "stack1",
            (i + 1) as u32,
            size as u32,
            Some("pr0"),
        ));
    }

    prs
}

/// Generate a diamond dependency stack (1→{2,3}→4)
fn generate_diamond_stack(layers: usize) -> Vec<Event> {
    let mut prs = Vec::new();

    // Base layer
    prs.push(create_test_pr("pr0", "stack1", 1, 4, None));

    // Middle layers (depend on base)
    for i in 1..layers - 1 {
        let event_id = format!("pr{}", i);
        prs.push(create_test_pr(
            &event_id,
            "stack1",
            (i + 1) as u32,
            layers as u32,
            Some("pr0"),
        ));
    }

    // Top layer (depends on last middle layer for benchmarking purposes)
    if layers > 2 {
        prs.push(create_test_pr(
            &format!("pr{}", layers - 1),
            "stack1",
            layers as u32,
            layers as u32,
            Some(&format!("pr{}", layers - 2)),
        ));
    }

    prs
}

fn bench_graph_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_construction");

    for size in [3, 10, 25, 50].iter() {
        let prs = generate_linear_stack(*size);

        group.bench_with_input(BenchmarkId::new("linear", size), &prs, |b, prs| {
            b.iter(|| {
                StackGraph::from_pr_events(black_box(prs)).unwrap();
            });
        });
    }

    group.finish();
}

fn bench_topological_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("topological_sort");

    // Linear stack
    for size in [3, 10, 25, 50].iter() {
        let prs = generate_linear_stack(*size);
        let graph = StackGraph::from_pr_events(&prs).unwrap();

        group.bench_with_input(BenchmarkId::new("linear", size), &graph, |b, graph| {
            b.iter(|| {
                black_box(graph.topological_sort().unwrap());
            });
        });
    }

    // Fan-out stack
    for size in [3, 10, 25, 50].iter() {
        let prs = generate_fanout_stack(*size);
        let graph = StackGraph::from_pr_events(&prs).unwrap();

        group.bench_with_input(BenchmarkId::new("fanout", size), &graph, |b, graph| {
            b.iter(|| {
                black_box(graph.topological_sort().unwrap());
            });
        });
    }

    // Diamond stack
    for size in [4, 10, 25].iter() {
        let prs = generate_diamond_stack(*size);
        let graph = StackGraph::from_pr_events(&prs).unwrap();

        group.bench_with_input(BenchmarkId::new("diamond", size), &graph, |b, graph| {
            b.iter(|| {
                black_box(graph.topological_sort().unwrap());
            });
        });
    }

    group.finish();
}

fn bench_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation");

    for size in [3, 10, 25, 50].iter() {
        let prs = generate_linear_stack(*size);
        let graph = StackGraph::from_pr_events(&prs).unwrap();

        group.bench_with_input(BenchmarkId::new("linear", size), &graph, |b, graph| {
            b.iter(|| {
                black_box(graph.validate().unwrap());
            });
        });
    }

    group.finish();
}

fn bench_get_stack_layers(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_stack_layers");

    for size in [3, 10, 25, 50].iter() {
        let prs = generate_linear_stack(*size);
        let graph = StackGraph::from_pr_events(&prs).unwrap();

        group.bench_with_input(BenchmarkId::new("size", size), &graph, |b, graph| {
            b.iter(|| {
                black_box(graph.get_stack_layers("stack1"));
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_graph_construction,
    bench_topological_sort,
    bench_validation,
    bench_get_stack_layers
);
criterion_main!(benches);
