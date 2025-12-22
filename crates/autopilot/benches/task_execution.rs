//! Benchmarks for autopilot task execution performance
//!
//! Run with: cargo bench -p autopilot --bench task_execution

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use issues::{db, issue, IssueType, Priority, Status};
use rusqlite::Connection;
use std::hint::black_box;
use tempfile::TempDir;

/// Setup a test database with N issues
fn setup_db_with_issues(n: usize) -> (TempDir, Connection) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("bench.db");
    let conn = db::init_db(&db_path).expect("Failed to init DB");

    // Create N issues
    for i in 0..n {
        let priority = match i % 4 {
            0 => Priority::Low,
            1 => Priority::Medium,
            2 => Priority::High,
            _ => Priority::Urgent,
        };
        let issue_type = match i % 3 {
            0 => IssueType::Task,
            1 => IssueType::Bug,
            _ => IssueType::Feature,
        };

        issue::create_issue(
            &conn,
            &format!("Benchmark issue {}", i),
            Some(&format!("Description for issue {}", i)),
            priority,
            issue_type,
            Some("claude"),
            None,
        )
        .expect("Failed to create issue");
    }

    (dir, conn)
}

/// Benchmark issue claim operation
fn bench_issue_claim(c: &mut Criterion) {
    let (_dir, conn) = setup_db_with_issues(1000);

    // Get open issues
    let issues = issue::list_issues(&conn, Some(Status::Open)).expect("Failed to list");
    let mut idx = 0;

    c.bench_function("issue_claim", |b| {
        b.iter(|| {
            if idx >= issues.len() {
                idx = 0;
            }
            let id = &issues[idx].id;
            idx += 1;

            issue::claim_issue(black_box(&conn), black_box(id), black_box("bench-run"))
                .expect("Failed to claim issue")
        });
    });
}

/// Benchmark issue completion operation
fn bench_issue_complete(c: &mut Criterion) {
    let (_dir, conn) = setup_db_with_issues(1000);

    // Claim all issues first
    let issues = issue::list_issues(&conn, Some(Status::Open)).expect("Failed to list");
    for iss in &issues {
        issue::claim_issue(&conn, &iss.id, "bench-run").expect("Failed to claim");
    }

    let mut idx = 0;

    c.bench_function("issue_complete", |b| {
        b.iter(|| {
            if idx >= issues.len() {
                idx = 0;
            }
            let id = &issues[idx].id;
            idx += 1;

            issue::complete_issue(black_box(&conn), black_box(id))
                .expect("Failed to complete issue")
        });
    });
}

/// Benchmark issue blocking operation
fn bench_issue_block(c: &mut Criterion) {
    let (_dir, conn) = setup_db_with_issues(1000);

    let issues = issue::list_issues(&conn, Some(Status::Open)).expect("Failed to list");
    let mut idx = 0;

    c.bench_function("issue_block", |b| {
        b.iter(|| {
            if idx >= issues.len() {
                idx = 0;
            }
            let id = &issues[idx].id;
            idx += 1;

            issue::block_issue(
                black_box(&conn),
                black_box(id),
                black_box("Benchmark blocking"),
            )
            .expect("Failed to block issue")
        });
    });
}

/// Benchmark getting next ready issue
fn bench_get_next_ready_issue(c: &mut Criterion) {
    let (_dir, conn) = setup_db_with_issues(1000);

    c.bench_function("get_next_ready_issue", |b| {
        b.iter(|| {
            issue::get_next_ready_issue(black_box(&conn), black_box(None))
                .expect("Failed to get ready issue")
        });
    });
}

/// Benchmark listing issues with different database sizes
fn bench_list_issues_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_issues_scaling");

    for size in [100, 500, 1000, 5000].iter() {
        let (_dir, conn) = setup_db_with_issues(*size);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                issue::list_issues(black_box(&conn), black_box(None))
                    .expect("Failed to list issues")
            });
        });
    }

    group.finish();
}

/// Benchmark filtered list queries
fn bench_filtered_list(c: &mut Criterion) {
    let (_dir, conn) = setup_db_with_issues(1000);

    let mut group = c.benchmark_group("filtered_list");

    group.bench_function("filter_by_open", |b| {
        b.iter(|| {
            issue::list_issues(black_box(&conn), black_box(Some(Status::Open)))
                .expect("Failed to list issues")
        });
    });

    // Claim some issues
    let issues = issue::list_issues(&conn, Some(Status::Open)).expect("Failed to list");
    for i in 0..500 {
        issue::claim_issue(&conn, &issues[i].id, "bench-run").expect("Failed to claim");
    }

    group.bench_function("filter_by_in_progress", |b| {
        b.iter(|| {
            issue::list_issues(black_box(&conn), black_box(Some(Status::InProgress)))
                .expect("Failed to list issues")
        });
    });

    // Complete some issues
    for i in 0..250 {
        issue::complete_issue(&conn, &issues[i].id).expect("Failed to complete");
    }

    group.bench_function("filter_by_done", |b| {
        b.iter(|| {
            issue::list_issues(black_box(&conn), black_box(Some(Status::Done)))
                .expect("Failed to list issues")
        });
    });

    group.finish();
}

/// Benchmark issue lookup by ID
fn bench_get_issue_by_id(c: &mut Criterion) {
    let (_dir, conn) = setup_db_with_issues(1000);

    let issues = issue::list_issues(&conn, None).expect("Failed to list");
    let test_id = issues[500].id.clone();

    c.bench_function("get_issue_by_id", |b| {
        b.iter(|| {
            issue::get_issue_by_id(black_box(&conn), black_box(&test_id))
                .expect("Failed to get issue")
        });
    });
}

/// Benchmark issue lookup by number
fn bench_get_issue_by_number(c: &mut Criterion) {
    let (_dir, conn) = setup_db_with_issues(1000);

    c.bench_function("get_issue_by_number", |b| {
        b.iter(|| {
            issue::get_issue_by_number(black_box(&conn), black_box(500))
                .expect("Failed to get issue")
        });
    });
}

/// Benchmark issue creation
fn bench_issue_creation(c: &mut Criterion) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("bench.db");
    let conn = db::init_db(&db_path).expect("Failed to init DB");

    let mut counter = 0;

    c.bench_function("issue_creation", |b| {
        b.iter(|| {
            counter += 1;
            issue::create_issue(
                black_box(&conn),
                black_box(&format!("Benchmark issue {}", counter)),
                black_box(Some("Test description")),
                black_box(Priority::Medium),
                black_box(IssueType::Task),
                black_box(Some("claude")),
                black_box(None),
            )
            .expect("Failed to create issue")
        });
    });
}

/// Benchmark concurrent issue claims (simulated)
fn bench_concurrent_claims(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_claims");

    for num_issues in [10, 50, 100].iter() {
        let (_dir, conn) = setup_db_with_issues(*num_issues);
        let issues = issue::list_issues(&conn, Some(Status::Open)).expect("Failed to list");

        group.bench_with_input(
            BenchmarkId::from_parameter(num_issues),
            num_issues,
            |b, _| {
                b.iter(|| {
                    // Simulate claiming all available issues in sequence
                    for iss in &issues {
                        issue::claim_issue(black_box(&conn), &iss.id, "bench-run")
                            .expect("Failed to claim");
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark issue update operations
fn bench_issue_update(c: &mut Criterion) {
    let (_dir, conn) = setup_db_with_issues(1000);

    let issues = issue::list_issues(&conn, None).expect("Failed to list");
    let test_id = issues[500].id.clone();

    c.bench_function("issue_update", |b| {
        b.iter(|| {
            issue::update_issue(
                black_box(&conn),
                black_box(&test_id),
                black_box(Some("Updated title")),
                black_box(Some("Updated description")),
                black_box(Some(Priority::High)),
                black_box(Some(IssueType::Bug)),
            )
            .expect("Failed to update issue")
        });
    });
}

/// Benchmark database initialization
fn bench_db_init(c: &mut Criterion) {
    c.bench_function("db_init", |b| {
        b.iter(|| {
            let dir = TempDir::new().expect("Failed to create temp dir");
            let db_path = dir.path().join("bench.db");
            db::init_db(black_box(&db_path)).expect("Failed to init DB")
        });
    });
}

criterion_group!(
    benches,
    bench_issue_claim,
    bench_issue_complete,
    bench_issue_block,
    bench_get_next_ready_issue,
    bench_list_issues_scaling,
    bench_filtered_list,
    bench_get_issue_by_id,
    bench_get_issue_by_number,
    bench_issue_creation,
    bench_concurrent_claims,
    bench_issue_update,
    bench_db_init,
);
criterion_main!(benches);
