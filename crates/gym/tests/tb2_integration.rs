//! Integration test for TB2 Docker integration

use gym::mechacoder::tb2_loader::TB2TaskLoader;
use gym::mechacoder::docker_runner::DockerRunner;
use sandbox::ContainerBackend;

#[tokio::test]
async fn test_tb2_task_discovery() {
    let loader = TB2TaskLoader::new_default();

    if !loader.is_available() {
        eprintln!("TB2 directory not found, skipping test");
        return;
    }

    let tasks = loader.discover_tasks();
    println!("Discovered {} TB2 tasks", tasks.len());
    assert!(!tasks.is_empty(), "Should discover at least one task");

    // Print first few tasks
    for task in tasks.iter().take(5) {
        println!("  - {} [{}] ({})", task.id, task.difficulty, task.category);
    }
}

#[tokio::test]
async fn test_load_regex_log_task() {
    let loader = TB2TaskLoader::new_default();

    if !loader.is_available() {
        eprintln!("TB2 directory not found, skipping test");
        return;
    }

    let task = loader.load_task("regex-log").expect("Should load regex-log task");

    println!("Task: {}", task.name);
    println!("Docker image: {}", task.docker_image());
    println!("Timeout: {}s", task.agent_timeout_sec());
    println!("Instruction length: {} chars", task.instruction.len());

    assert_eq!(task.id, "regex-log");
    assert!(!task.instruction.is_empty());
    assert!(task.docker_image().contains("regex-log"));
}

#[tokio::test]
async fn test_docker_available() {
    let runner = DockerRunner::new();
    let available = runner.is_available().await;

    println!("Docker available: {}", available);
    assert!(available, "Docker should be available for TB2 tests");
}

#[tokio::test]
#[ignore] // Run with --ignored flag - pulls image
async fn test_pull_regex_log_image() {
    let loader = TB2TaskLoader::new_default();

    if !loader.is_available() {
        eprintln!("TB2 directory not found, skipping test");
        return;
    }

    let task = loader.load_task("regex-log").expect("Should load task");
    let runner = DockerRunner::new();

    println!("Pulling image: {}", task.docker_image());
    runner.ensure_image(task.docker_image()).await.expect("Should pull image");
    println!("Image pulled successfully");
}
