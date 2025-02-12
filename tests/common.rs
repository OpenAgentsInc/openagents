use sqlx::PgPool;

pub async fn setup_test_db() -> PgPool {
    println!("Setting up test database...");

    // Use a dedicated test database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/openagents_test".to_string());
    println!("Using database URL: {}", database_url);

    // Create a connection to the default postgres database to create our test db
    let admin_url = "postgres://postgres:postgres@localhost:5432/postgres";
    println!("Connecting to admin database...");
    let admin_pool = PgPool::connect(admin_url)
        .await
        .expect("Failed to connect to postgres database");

    // Drop the test database if it exists and recreate it
    println!("Dropping existing test database...");
    let _ = sqlx::query("DROP DATABASE IF EXISTS openagents_test")
        .execute(&admin_pool)
        .await;

    println!("Creating test database...");
    sqlx::query("CREATE DATABASE openagents_test")
        .execute(&admin_pool)
        .await
        .expect("Failed to create test database");

    // Connect to the new test database
    println!("Connecting to test database...");
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    // List available migrations
    println!("Available migrations:");
    for entry in std::fs::read_dir("./migrations").unwrap() {
        let entry = entry.unwrap();
        println!("  {}", entry.file_name().to_string_lossy());
    }

    // Run migrations
    println!("Running migrations...");
    match sqlx::migrate!("./migrations").run(&pool).await {
        Ok(_) => println!("Migrations completed successfully"),
        Err(e) => println!("Migration error: {}", e),
    }

    // Verify tables
    println!("Verifying tables...");
    let tables = sqlx::query("SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'")
        .fetch_all(&pool)
        .await
        .expect("Failed to query tables");

    println!("Available tables:");
    for row in tables {
        let table_name: String = row.get(0);
        println!("  {}", table_name);
    }

    pool
}
