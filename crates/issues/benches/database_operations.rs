//! Benchmarks for issue database operations
//!
//! Run with: cargo bench -p issues --bench database_operations

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use issues::{IssueType, Priority, db, issue};
use rusqlite::Connection;
use tempfile::TempDir;

/// Setup a test database with N issues
fn setup_db_with_issues(n: usize) -> (TempDir, Connection) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("bench.db");
    let conn = db::init_db(&db_path).expect("Failed to init DB");

    // Create N issues with varying properties
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
        let agent = if i % 2 == 0 { "claude" } else { "codex" };

        issue::create_issue(
            &conn,
            &format!("Benchmark issue {}", i),
            Some(&format!("Description for issue {}", i)),
            priority,
            issue_type,
            Some(agent),
            None,
        )
        .expect("Failed to create issue");
    }

    (dir, conn)
}

fn bench_create_issue(c: &mut Criterion) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("bench.db");
    let conn = db::init_db(&db_path).expect("Failed to init DB");

    c.bench_function("create_issue", |b| {
        let mut counter = 0;
        b.iter(|| {
            counter += 1;
            issue::create_issue(
                black_box(&conn),
                black_box(&format!("Test issue {}", counter)),
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

fn bench_list_issues(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_issues");

    for size in [10, 100, 1000].iter() {
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

fn bench_list_issues_with_filter(c: &mut Criterion) {
    let (_dir, conn) = setup_db_with_issues(1000);

    c.bench_function("list_issues_filtered_by_status", |b| {
        b.iter(|| {
            issue::list_issues(black_box(&conn), black_box(Some(issues::Status::Open)))
                .expect("Failed to list issues")
        });
    });
}

fn bench_get_issue_by_id(c: &mut Criterion) {
    let (_dir, conn) = setup_db_with_issues(1000);

    // Get a known issue ID
    let issues = issue::list_issues(&conn, None).expect("Failed to list");
    let test_id = issues[500].id.clone();

    c.bench_function("get_issue_by_id", |b| {
        b.iter(|| {
            issue::get_issue_by_id(black_box(&conn), black_box(&test_id))
                .expect("Failed to get issue")
        });
    });
}

fn bench_get_issue_by_number(c: &mut Criterion) {
    let (_dir, conn) = setup_db_with_issues(1000);

    c.bench_function("get_issue_by_number", |b| {
        b.iter(|| {
            issue::get_issue_by_number(black_box(&conn), black_box(500))
                .expect("Failed to get issue")
        });
    });
}

fn bench_claim_issue(c: &mut Criterion) {
    let (_dir, conn) = setup_db_with_issues(1000);

    let issues = issue::list_issues(&conn, Some(issues::Status::Open)).expect("Failed to list");
    let mut issue_idx = 0;

    c.bench_function("claim_issue", |b| {
        b.iter(|| {
            if issue_idx >= issues.len() {
                issue_idx = 0;
            }
            let id = &issues[issue_idx].id;
            issue_idx += 1;

            issue::claim_issue(black_box(&conn), black_box(id), black_box("bench-run"))
                .expect("Failed to claim issue")
        });
    });
}

fn bench_complete_issue(c: &mut Criterion) {
    let (_dir, conn) = setup_db_with_issues(1000);

    // Claim all issues first
    let issues = issue::list_issues(&conn, Some(issues::Status::Open)).expect("Failed to list");
    for iss in &issues {
        issue::claim_issue(&conn, &iss.id, "bench-run").expect("Failed to claim");
    }

    let mut issue_idx = 0;

    c.bench_function("complete_issue", |b| {
        b.iter(|| {
            if issue_idx >= issues.len() {
                issue_idx = 0;
            }
            let id = &issues[issue_idx].id;
            issue_idx += 1;

            issue::complete_issue(black_box(&conn), black_box(id))
                .expect("Failed to complete issue")
        });
    });
}

fn bench_block_issue(c: &mut Criterion) {
    let (_dir, conn) = setup_db_with_issues(1000);

    let issues = issue::list_issues(&conn, Some(issues::Status::Open)).expect("Failed to list");
    let mut issue_idx = 0;

    c.bench_function("block_issue", |b| {
        b.iter(|| {
            if issue_idx >= issues.len() {
                issue_idx = 0;
            }
            let id = &issues[issue_idx].id;
            issue_idx += 1;

            issue::block_issue(
                black_box(&conn),
                black_box(id),
                black_box("Benchmark blocking"),
            )
            .expect("Failed to block issue")
        });
    });
}

fn bench_get_next_ready_issue(c: &mut Criterion) {
    let (_dir, conn) = setup_db_with_issues(1000);

    c.bench_function("get_next_ready_issue", |b| {
        b.iter(|| {
            issue::get_next_ready_issue(black_box(&conn), black_box(None))
                .expect("Failed to get ready issue")
        });
    });
}

criterion_group!(
    benches,
    bench_create_issue,
    bench_list_issues,
    bench_list_issues_with_filter,
    bench_get_issue_by_id,
    bench_get_issue_by_number,
    bench_claim_issue,
    bench_complete_issue,
    bench_block_issue,
    bench_get_next_ready_issue,
);
criterion_main!(benches);
