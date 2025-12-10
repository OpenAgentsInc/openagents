# Effect Patterns in Rust: Analysis & Strategy

> **Created:** 2025-12-09
> **Purpose:** Analyze to what extent we can/should mimic Effect TypeScript patterns in Rust
> **Prerequisites:** Understanding of Effect TypeScript and Rust functional programming

---

## Table of Contents

1. [Overview](#overview)
2. [Rust Functional Programming Capabilities](#rust-functional-programming-capabilities)
3. [Effect Pattern Analysis](#effect-pattern-analysis)
4. [What to Port, What to Replace](#what-to-port-what-to-replace)
5. [Recommended Approach](#recommended-approach)
6. [Implementation Examples](#implementation-examples)
7. [Decision Matrix](#decision-matrix)

---

## Overview

Effect TypeScript provides:
- **Composable effects** - `Effect<T, E, R>` for lazy async computation
- **Context/services** - Dependency injection via `Context.Tag`
- **Layer composition** - Building dependency graphs
- **Stream processing** - Reactive data streams
- **Error handling** - Tagged errors with type safety
- **Generator syntax** - `Effect.gen` for imperative-style async

Rust provides:
- **Zero-cost abstractions** - Functional patterns compile to optimal code
- **Closures** - Function-like values with environment capture
- **Iterators** - Lazy, composable data transformation
- **`async/await`** - Native async with `Future` trait
- **Result/Option** - Type-safe error handling
- **Trait-based polymorphism** - Zero-cost dependency injection

**Core Question:** Should we build an Effect-like system in Rust, or embrace Rust's native patterns?

---

## Rust Functional Programming Capabilities

Based on [The Rust Book Chapter 13](https://doc.rust-lang.org/book/ch13-00-functional-features.html):

### 1. Closures

**Capabilities:**
- Anonymous functions that capture environment
- Type inference (no explicit signatures needed)
- Three capture modes: immutable borrow, mutable borrow, move
- Implements `Fn`, `FnMut`, or `FnOnce` traits automatically

**Example:**
```rust
let multiplier = 5;
let multiply = |x| x * multiplier;  // Captures multiplier
println!("{}", multiply(3));  // 15
```

**Performance:** Zero-cost abstraction - compiles to same code as hand-written functions.

### 2. Iterators

**Capabilities:**
- Lazy evaluation (nothing happens until consumed)
- Composable adapters (`map`, `filter`, `flat_map`, etc.)
- Type-safe chaining
- Zero-cost - compiles to same code as loops

**Example:**
```rust
let result: Vec<_> = vec![1, 2, 3, 4]
    .iter()
    .map(|x| x * 2)
    .filter(|x| x > 4)
    .collect();
// result = [6, 8]
```

**Performance:** Benchmarks show identical performance to hand-written loops.

### 3. Result & Option

**Capabilities:**
- Type-safe error handling
- Monadic operations (`map`, `and_then`, `or_else`)
- `?` operator for error propagation
- Pattern matching for exhaustiveness

**Example:**
```rust
fn divide(a: i32, b: i32) -> Result<i32, String> {
    if b == 0 {
        Err("Division by zero".to_string())
    } else {
        Ok(a / b)
    }
}

let result = divide(10, 2)?
    .checked_add(5)
    .ok_or("Overflow")?;
```

### 4. Async/Await

**Capabilities:**
- Native async with `Future` trait
- Composable async operations
- Works with standard library
- Multiple runtimes (Tokio, async-std)

**Example:**
```rust
async fn fetch_data() -> Result<String, Error> {
    let response = reqwest::get("https://api.example.com")
        .await?;
    let text = response.text().await?;
    Ok(text)
}
```

### 5. Trait-Based Polymorphism

**Capabilities:**
- Zero-cost abstraction (static dispatch)
- Multiple trait bounds
- Associated types
- Generic constraints

**Example:**
```rust
trait DataSource {
    async fn fetch(&self, id: u32) -> Result<Data, Error>;
}

async fn process<T: DataSource>(source: &T, id: u32) -> Result<(), Error> {
    let data = source.fetch(id).await?;
    // Process data
    Ok(())
}
```

---

## Effect Pattern Analysis

Let's analyze each Effect pattern and how it maps to Rust:

### Pattern 1: Effect<T, E, R>

**Effect TypeScript:**
```typescript
Effect.gen(function* () {
  const user = yield* getUserById(123)
  const posts = yield* getPostsByUser(user.id)
  return posts
})
```

**Rust Options:**

**Option A: Native async/await (Recommended)**
```rust
async fn get_posts_for_user(id: u32) -> Result<Vec<Post>, Error> {
    let user = get_user_by_id(id).await?;
    let posts = get_posts_by_user(user.id).await?;
    Ok(posts)
}
```

**Option B: Custom Effect type (More work)**
```rust
// Define Effect type
struct Effect<T, E, R> {
    run: Box<dyn FnOnce(&R) -> Result<T, E>>,
}

// Usage
let effect = Effect::gen(|_ctx| async {
    let user = get_user_by_id(123).await?;
    let posts = get_posts_by_user(user.id).await?;
    Ok(posts)
});
```

**Verdict:** Use native `async/await` with `Result<T, E>`. Rust's async is already an effect system.

### Pattern 2: Context/Services

**Effect TypeScript:**
```typescript
interface UserService extends Context.Tag<"UserService", UserService> {
  getUser: (id: number) => Effect<User, Error>
}

Effect.gen(function* () {
  const userService = yield* UserService
  const user = yield* userService.getUser(123)
  return user
})
```

**Rust Options:**

**Option A: Trait + Generics (Recommended)**
```rust
trait UserService {
    async fn get_user(&self, id: u32) -> Result<User, Error>;
}

async fn get_user_data<U: UserService>(service: &U, id: u32) -> Result<User, Error> {
    service.get_user(id).await
}
```

**Option B: Trait Objects (Dynamic dispatch)**
```rust
struct App {
    user_service: Arc<dyn UserService>,
}

impl App {
    async fn get_user_data(&self, id: u32) -> Result<User, Error> {
        self.user_service.get_user(id).await
    }
}
```

**Option C: Custom Context type**
```rust
struct Context {
    user_service: Arc<dyn UserService>,
    db_service: Arc<dyn DbService>,
}

async fn get_user_data(ctx: &Context, id: u32) -> Result<User, Error> {
    ctx.user_service.get_user(id).await
}
```

**Verdict:** Use traits with generics for static dispatch (zero-cost), or trait objects for flexibility. No need for complex DI framework.

### Pattern 3: Layer Composition

**Effect TypeScript:**
```typescript
const UserServiceLive = Layer.succeed(UserService, new UserServiceImpl())
const DbServiceLive = Layer.effect(DbService, makeDbService)
const AppLayer = Layer.provide(UserServiceLive, DbServiceLive)

const program = Effect.provide(myProgram, AppLayer)
```

**Rust Options:**

**Option A: Builder Pattern (Recommended)**
```rust
struct App {
    user_service: Arc<dyn UserService>,
    db_service: Arc<dyn DbService>,
}

impl App {
    fn builder() -> AppBuilder { AppBuilder::new() }
}

struct AppBuilder {
    user_service: Option<Arc<dyn UserService>>,
    db_service: Option<Arc<dyn DbService>>,
}

impl AppBuilder {
    fn with_user_service(mut self, service: Arc<dyn UserService>) -> Self {
        self.user_service = Some(service);
        self
    }

    fn build(self) -> App {
        App {
            user_service: self.user_service.unwrap(),
            db_service: self.db_service.unwrap(),
        }
    }
}

// Usage
let app = App::builder()
    .with_user_service(Arc::new(UserServiceImpl))
    .with_db_service(Arc::new(DbServiceImpl))
    .build();
```

**Option B: Function Composition**
```rust
type ServiceFactory<T> = Box<dyn FnOnce() -> T>;

fn provide_user_service() -> Arc<dyn UserService> {
    Arc::new(UserServiceImpl)
}

fn provide_db_service() -> Arc<dyn DbService> {
    Arc::new(DbServiceImpl)
}

// Composition
let app = App {
    user_service: provide_user_service(),
    db_service: provide_db_service(),
};
```

**Verdict:** Use builder pattern or simple composition. Rust's type system + builder pattern gives you compile-time dependency checking.

### Pattern 4: Stream Processing

**Effect TypeScript:**
```typescript
pipe(
  Stream.fromIterable([1, 2, 3]),
  Stream.map(x => x * 2),
  Stream.filter(x => x > 2),
  Stream.runCollect
)
```

**Rust Options:**

**Option A: Iterator (Recommended)**
```rust
let result: Vec<_> = vec![1, 2, 3]
    .into_iter()
    .map(|x| x * 2)
    .filter(|x| x > 2)
    .collect();
```

**Option B: tokio-stream (for async)**
```rust
use tokio_stream::{self as stream, StreamExt};

let result = stream::iter(vec![1, 2, 3])
    .map(|x| x * 2)
    .filter(|x| x > 2)
    .collect::<Vec<_>>()
    .await;
```

**Verdict:** Use native `Iterator` for sync, `tokio-stream` for async. Both are zero-cost and composable.

### Pattern 5: Error Handling

**Effect TypeScript:**
```typescript
class NotFoundError extends Data.TaggedError("NotFoundError")<{
  id: number
}> {}

Effect.fail(new NotFoundError({ id: 123 }))
  .pipe(Effect.catchTag("NotFoundError", (e) => Effect.succeed(null)))
```

**Rust Options:**

**Option A: thiserror (Recommended)**
```rust
use thiserror::Error;

#[derive(Debug, Error)]
enum AppError {
    #[error("User not found: {id}")]
    NotFound { id: u32 },

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

// Usage
fn get_user(id: u32) -> Result<User, AppError> {
    Err(AppError::NotFound { id })
}

// Pattern matching
match get_user(123) {
    Ok(user) => println!("Found: {}", user.name),
    Err(AppError::NotFound { id }) => println!("Not found: {}", id),
    Err(e) => println!("Error: {}", e),
}
```

**Option B: anyhow (for application-level)**
```rust
use anyhow::{Context, Result};

fn get_user(id: u32) -> Result<User> {
    let user = db.get(id)
        .context("Failed to fetch user")?;
    Ok(user)
}
```

**Verdict:** Use `thiserror` for library errors, `anyhow` for application errors. Pattern matching gives exhaustive error handling.

---

## What to Port, What to Replace

### ✅ Port (Conceptually)

These Effect concepts have clear Rust equivalents:

| Effect Pattern | Rust Equivalent | Notes |
|----------------|-----------------|-------|
| `Effect<T, E, R>` | `async fn() -> Result<T, E>` | Native async is an effect system |
| `Effect.gen` | `async { }` | Async blocks replace generators |
| `Result<T, E>` | `Result<T, E>` | Direct 1:1 mapping |
| `Option<T>` | `Option<T>` | Direct 1:1 mapping |
| `Stream<T>` | `Iterator<T>` or `Stream<T>` | Iterators for sync, tokio-stream for async |
| Service injection | Traits + generics | Zero-cost abstraction |
| `pipe` | Method chaining | `.map().filter().collect()` |

### ❌ Don't Port (Use Rust Idioms)

These Effect patterns should use Rust's native features:

| Effect Pattern | Why Not Port | Rust Alternative |
|----------------|--------------|------------------|
| `Context.Tag` system | Unnecessary complexity | Trait bounds + generics |
| `Layer` composition | Over-engineered | Builder pattern |
| `Effect.gen` syntax | We have `async/await` | Native async blocks |
| Tagged errors (complex) | Pattern matching is better | `thiserror` enums |
| `Scope` for resource management | RAII is superior | Drop trait |
| `Fiber` system | We have Tokio tasks | `tokio::spawn` |

### 🤔 Consider (If Strong Need)

These might be worth a lightweight implementation:

| Pattern | Use Case | Implementation Effort |
|---------|----------|----------------------|
| Retry logic | Common pattern | Medium (create a reusable crate) |
| Circuit breaker | Microservices | Medium (use existing crate) |
| Rate limiting | API calls | Low (use existing crate) |
| Timeout combinators | Async operations | Low (Tokio provides this) |

---

## Recommended Approach

### For OpenAgents Migration

**Philosophy:** Embrace Rust idioms, don't fight them. Use Effect *concepts* but Rust *implementation*.

### Core Principles

1. **Use `async/await` instead of Effect.gen**
   - More readable
   - Better error messages
   - Native tooling support

2. **Use `Result<T, E>` instead of Effect<T, E, R>**
   - Standard library
   - `?` operator for propagation
   - Pattern matching for handling

3. **Use traits instead of Context.Tag**
   - Zero-cost abstraction
   - Compile-time checking
   - No magic DI framework

4. **Use builder pattern instead of Layers**
   - Standard Rust pattern
   - Type-safe composition
   - Clear dependency flow

5. **Use iterators/streams instead of Effect.Stream**
   - Zero-cost
   - Lazy evaluation
   - Standard library

### Architecture Pattern

```rust
// Define services as traits
trait UserRepository: Send + Sync {
    async fn get(&self, id: u32) -> Result<User, Error>;
    async fn save(&self, user: User) -> Result<(), Error>;
}

trait NotificationService: Send + Sync {
    async fn send(&self, user_id: u32, message: String) -> Result<(), Error>;
}

// Application struct holds service instances
struct App {
    user_repo: Arc<dyn UserRepository>,
    notification: Arc<dyn NotificationService>,
}

impl App {
    // Business logic methods
    async fn register_user(&self, data: UserData) -> Result<User, Error> {
        let user = User::new(data);
        self.user_repo.save(user.clone()).await?;
        self.notification.send(user.id, "Welcome!".to_string()).await?;
        Ok(user)
    }
}

// Builder for dependency injection
impl App {
    fn builder() -> AppBuilder {
        AppBuilder::default()
    }
}

struct AppBuilder {
    user_repo: Option<Arc<dyn UserRepository>>,
    notification: Option<Arc<dyn NotificationService>>,
}

impl AppBuilder {
    fn with_user_repo(mut self, repo: Arc<dyn UserRepository>) -> Self {
        self.user_repo = Some(repo);
        self
    }

    fn with_notification(mut self, service: Arc<dyn NotificationService>) -> Self {
        self.notification = Some(service);
        self
    }

    fn build(self) -> Result<App, String> {
        Ok(App {
            user_repo: self.user_repo.ok_or("User repo required")?,
            notification: self.notification.ok_or("Notification service required")?,
        })
    }
}

// Usage
let app = App::builder()
    .with_user_repo(Arc::new(PostgresUserRepo::new(db_pool)))
    .with_notification(Arc::new(EmailNotification::new(smtp_config)))
    .build()?;

app.register_user(user_data).await?;
```

---

## Implementation Examples

### Example 1: Effect Pipeline → Rust

**Effect TypeScript:**
```typescript
Effect.gen(function* () {
  const fs = yield* FileSystem.FileSystem
  const content = yield* fs.readFileString(path)
  const lines = content.split('\n')
  const filtered = lines.filter(line => line.includes("TODO"))
  return filtered
}).pipe(Effect.provide(FileSystemLive))
```

**Rust:**
```rust
async fn find_todos(path: &Path) -> Result<Vec<String>, Error> {
    let content = tokio::fs::read_to_string(path).await?;
    let filtered: Vec<_> = content
        .lines()
        .filter(|line| line.contains("TODO"))
        .map(|s| s.to_string())
        .collect();
    Ok(filtered)
}
```

**Improvements:**
- Simpler, more direct
- No DI framework needed
- Better error messages
- Easier to debug

### Example 2: Service Injection

**Effect TypeScript:**
```typescript
interface DbService extends Context.Tag<"DbService", DbService> {
  query: (sql: string) => Effect<Row[], Error>
}

const program = Effect.gen(function* () {
  const db = yield* DbService
  const rows = yield* db.query("SELECT * FROM users")
  return rows
})

const provided = Effect.provide(program, DbServiceLive)
```

**Rust:**
```rust
trait DbService: Send + Sync {
    async fn query(&self, sql: &str) -> Result<Vec<Row>, Error>;
}

async fn get_users<D: DbService>(db: &D) -> Result<Vec<Row>, Error> {
    db.query("SELECT * FROM users").await
}

// Or with trait object
struct App {
    db: Arc<dyn DbService>,
}

impl App {
    async fn get_users(&self) -> Result<Vec<Row>, Error> {
        self.db.query("SELECT * FROM users").await
    }
}
```

### Example 3: Error Handling

**Effect TypeScript:**
```typescript
class ValidationError extends Data.TaggedError("ValidationError") {}
class DatabaseError extends Data.TaggedError("DatabaseError") {}

Effect.fail(new ValidationError())
  .pipe(
    Effect.catchTag("ValidationError", () => Effect.succeed(null)),
    Effect.catchTag("DatabaseError", (e) => Effect.fail(e))
  )
```

**Rust:**
```rust
#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

fn process() -> Result<Option<Data>, AppError> {
    Err(AppError::Validation("Invalid input".to_string()))
}

// Pattern matching
match process() {
    Ok(data) => Ok(data),
    Err(AppError::Validation(_)) => Ok(None), // Handle validation error
    Err(e) => Err(e), // Propagate other errors
}
```

### Example 4: Retry Logic

**Effect TypeScript:**
```typescript
Effect.retry(fetchData, {
  schedule: Schedule.exponential(100),
  times: 3,
})
```

**Rust (with helper crate):**
```rust
use tokio_retry::{strategy::ExponentialBackoff, Retry};

async fn fetch_with_retry() -> Result<Data, Error> {
    let retry_strategy = ExponentialBackoff::from_millis(100).take(3);

    Retry::spawn(retry_strategy, || async {
        fetch_data().await
    }).await
}
```

**Or custom implementation:**
```rust
async fn retry<F, Fut, T, E>(mut f: F, max_retries: u32) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut attempts = 0;
    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) if attempts >= max_retries => return Err(e),
            Err(_) => {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(100 * 2u64.pow(attempts))).await;
            }
        }
    }
}

// Usage
retry(|| fetch_data(), 3).await?;
```

---

## Decision Matrix

When deciding whether to port an Effect pattern:

| Question | If Yes → | If No → |
|----------|----------|---------|
| Does Rust have a native equivalent? | Use Rust's version | Consider port |
| Is it a zero-cost abstraction in Rust? | Prefer Rust approach | Consider custom |
| Does it integrate with std library? | Use native | Custom wrapper |
| Is there an existing crate? | Use crate | Build if needed |
| Does it improve type safety? | Port concept | Skip |
| Does it add significant complexity? | Skip | Consider port |

### Specific Decisions

| Effect Feature | Port? | Rust Approach |
|----------------|-------|---------------|
| Effect<T, E, R> | ❌ | `async fn() -> Result<T, E>` |
| Effect.gen | ❌ | `async { }` blocks |
| Context.Tag | ❌ | Trait bounds |
| Layer | ❌ | Builder pattern |
| Stream | ⚠️ | Iterator/tokio-stream |
| Tagged errors | ✅ | `thiserror` enums |
| Retry | ✅ | Helper function/crate |
| Timeout | ✅ | `tokio::time::timeout` |
| Resource management | ❌ | RAII + Drop |

---

## Performance Considerations

### Zero-Cost Abstractions

Rust's functional features are **zero-cost**:

```rust
// Iterator approach
let sum: i32 = vec![1, 2, 3, 4]
    .iter()
    .map(|x| x * 2)
    .sum();

// Hand-written loop
let mut sum = 0;
for x in &vec![1, 2, 3, 4] {
    sum += x * 2;
}

// Both compile to identical assembly
```

**Benchmark results** (from Rust Book):
- Iterator: 19,234,900 ns
- Loop: 19,620,300 ns
- **Difference: Negligible**

### When to Use What

| Pattern | Performance | Use When |
|---------|-------------|----------|
| Iterator | ⚡ Fastest | Sequential data processing |
| async/await | ⚡ Fastest | I/O-bound operations |
| Trait generics | ⚡ Fastest | Static dispatch (monomorphization) |
| Trait objects | 🐢 Dynamic dispatch | Need runtime polymorphism |
| Custom Effect | 🐢 Overhead | Complex dependency graphs |

**Recommendation:** Prefer zero-cost abstractions (iterators, async, trait generics) over custom Effect system.

---

## Conclusion

### Summary

1. **Don't build Effect-TS in Rust** - Rust has better native alternatives
2. **Port concepts, not implementation** - Use Effect *ideas* with Rust *idioms*
3. **Embrace Rust's strengths**:
   - `async/await` for effects
   - `Result<T, E>` for error handling
   - Traits for dependency injection
   - Iterators for data transformation
   - RAII for resource management

4. **Use existing ecosystem**:
   - `tokio` for async runtime
   - `thiserror` for error types
   - `anyhow` for app-level errors
   - `tokio-stream` for async streams
   - `tokio-retry` for retry logic

### For OpenAgents

**Recommended approach:**

1. Start with standard Rust patterns (async, Result, traits)
2. Create small helper utilities where Effect had nice APIs (retry, timeout)
3. Use builder pattern for dependency injection
4. Keep it simple - don't over-engineer

**This gives us:**
- ✅ Zero-cost abstractions
- ✅ Better compile errors
- ✅ Easier debugging
- ✅ Familiar to Rust developers
- ✅ Better tooling support
- ✅ Simpler mental model

**We lose:**
- ❌ Effect's elaborate DI system (but gain simplicity)
- ❌ Effect.gen syntax (but gain async/await)
- ❌ Layer composition (but gain builder pattern)

**Net result:** Simpler, faster, more maintainable Rust code.

---

**Last Updated:** 2025-12-09
**Status:** Recommendation
**See Also:**
- [rust-migration-plan.md](./rust-migration-plan.md)
- [Rust Book Chapter 13](https://doc.rust-lang.org/book/ch13-00-functional-features.html)
