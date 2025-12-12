# Tokio Reactor Architecture for TB2 Docker Runs

## The Problem: "No Reactor Running" Crash

When running TB2 tasks in MechaCoder, the system was crashing with:

```
thread 'main' panicked at tokio-1.48.0/src/process/unix/pidfd_reaper.rs:194:19:
there is no reactor running, must be called from the context of a Tokio 1.x runtime
```

This document explains **why** this happened and **how** we fixed it.

---

## Understanding Tokio Reactors

### What is a Reactor?

A Tokio **reactor** is the I/O event loop that powers async operations. It's responsible for:

1. **Process spawning** (via `tokio::process::Command`)
2. **Socket I/O** (TCP, UDP, Unix sockets)
3. **File I/O** (on Linux, using io_uring or epoll)
4. **Timers** (tokio::time::sleep, etc.)

On Linux, process spawning specifically requires **pidfd support** - a mechanism that uses file descriptors to monitor child processes.

### Where Reactors Live

Reactors are **thread-local**. They exist on:

- **Runtime worker threads** - Created by `Runtime::new()` or `Runtime::Builder::new_multi_thread()`
- **Current thread runtime** - Created by `Runtime::Builder::new_current_thread()` but has limited capabilities

Reactors do **NOT** exist on:
- Arbitrary `std::thread` threads
- GPUI's event loop threads (they have their own async system)

### The Critical Rule

**`tokio::process::Command` requires a reactor on the current thread's thread-local storage.**

```rust
// ❌ CRASHES - no reactor on std::thread
std::thread::spawn(|| {
    tokio::process::Command::new("docker").spawn() // PANIC!
});

// ❌ CRASHES - even with runtime, block_on runs on current thread
std::thread::spawn(|| {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        tokio::process::Command::new("docker").spawn() // PANIC!
    })
});

// ✅ WORKS - spawn runs on runtime worker thread
std::thread::spawn(|| {
    let rt = Runtime::new().unwrap();
    let handle = rt.spawn(async {
        tokio::process::Command::new("docker").spawn() // Has reactor!
    });
    rt.block_on(handle)
});
```

---

## MechaCoder's Architecture

### The GPUI Problem

MechaCoder uses **GPUI** for UI, which provides `cx.spawn()` for async work. GPUI has its own async executor that is **NOT** based on a full Tokio runtime with reactor support.

```rust
// GPUI's cx.spawn - NO REACTOR
cx.spawn(async move |_, cx| {
    // This context lacks a Tokio reactor
    // Cannot spawn Docker containers here!
})
```

### The Docker Runner Isolation

TB2 runs require:
1. **DockerRunner** - Spawns Claude CLI inside Docker containers
2. **TB2Verifier** - Runs test.sh in Docker containers

Both use `tokio::process::Command` internally to spawn Docker processes.

To provide reactor access, we use **complete runtime isolation**:

```rust
// Spawn dedicated thread with isolated Tokio runtime
std::thread::spawn(move || {
    // Create multi-threaded runtime
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    // Spawn work on worker threads (has reactor!)
    let handle = rt.spawn(async move {
        // All Docker operations happen HERE
        docker_runner.run_claude(...).await;
        verifier.run_tests(...).await;
    });

    // Block until complete
    rt.block_on(handle)
});
```

---

## The Original Bug

### Initial Broken Architecture

```
GymPanel: User clicks "Run TB2"
    ↓
Screen::handle_gym_panel_event()
    ↓
    ├─ std::thread::spawn {
    │      Runtime::new()
    │      rt.spawn {
    │          docker_runner.run_claude() ✅ Has reactor
    │      }
    │      rt.block_on(handle) → returns
    │  }
    │
    └─ cx.spawn {  ← GPUI context
           event_rx.recv()  ← Process events
           ...
           verifier.run_tests()  ❌ NO REACTOR - CRASH HERE
       }
```

### Why It Crashed

1. **DockerRunner succeeded** because it ran inside `rt.spawn()` on a worker thread with reactor
2. **Verifier failed** because it ran inside `cx.spawn()` (GPUI context) with no reactor
3. When `verifier.run_tests()` called `self.backend.run()` → `Command::new("docker")` → **PANIC**

The error message said "thread 'main'" because GPUI's executor runs on the main thread.

---

## The Fix

### New Architecture

```
GymPanel: User clicks "Run TB2"
    ↓
Screen::handle_gym_panel_event()
    ↓
    ├─ std::thread::spawn {
    │      Runtime::new()
    │      rt.spawn {
    │          // ALL Docker work in one place
    │          run_result = docker_runner.run_claude() ✅
    │          verification = verifier.run_tests()     ✅
    │
    │          // Send results via channel
    │          event_tx.send(DockerEvent::RunComplete {
    │              run_result,
    │              verification
    │          })
    │      }
    │      rt.block_on(handle)
    │  }
    │
    └─ cx.spawn {  ← GPUI context
           while let Some(event) = event_rx.recv() {
               match event {
                   DockerEvent::RunComplete { .. } => {
                       // Just update UI ✅ No reactor needed
                   }
               }
           }
       }
```

### Key Changes

1. **Moved verification INTO runtime.spawn**
   - Both `run_claude()` and `run_tests()` now execute on worker threads
   - Both have full reactor access

2. **Added RunComplete event**
   - Carries both Docker run results AND verification results
   - Sent via `tokio::sync::mpsc` unbounded channel

3. **GPUI handler just processes events**
   - No Docker operations
   - No reactor needed
   - Just UI updates

### Code Changes

#### 1. DockerEvent::RunComplete

```rust
// crates/mechacoder/src/panels/docker_runner.rs
pub enum DockerEvent {
    // ... existing variants ...

    RunComplete {
        run_result: Option<DockerRunResult>,
        run_error: Option<String>,
        verification: Option<VerificationResult>,
    },
}
```

#### 2. Runtime Worker Task

```rust
// crates/mechacoder/src/screen.rs
rt.spawn(async move {
    // Create Docker runner
    let docker_runner = DockerRunner::new();

    // Run Claude in Docker container
    let run_result = docker_runner.run_claude(&config, event_tx.clone(), abort_rx).await;

    // Run verification (NOW HAS REACTOR)
    let verification = if run_result.is_ok() {
        let verifier = TB2Verifier::new();
        verifier.run_tests(&task, &workspace, &logs).await.ok()
    } else {
        None
    };

    // Send completion via channel
    let (run_ok, run_err) = match run_result {
        Ok(r) => (Some(r), None),
        Err(e) => (None, Some(e.to_string())),
    };

    event_tx.send(DockerEvent::RunComplete {
        run_result: run_ok,
        run_error: run_err,
        verification,
    });
})
```

#### 3. GPUI Event Handler

```rust
// crates/mechacoder/src/screen.rs
cx.spawn(async move |_, cx| {
    // Just process events from channel
    while let Some(event) = event_rx.recv().await {
        if matches!(event, DockerEvent::RunComplete { .. }) {
            // Extract results and update UI
            // NO DOCKER OPERATIONS HERE
            break;
        }
        // Handle other events...
    }
})
```

---

## Why This Works

### 1. Runtime Isolation is Complete

The `std::thread` creates a completely independent Tokio runtime:
- Has its own worker thread pool
- Has its own I/O driver (reactor)
- Has its own timer wheel
- Completely separate from GPUI's executor

### 2. Worker Threads Have Reactors

When you call `rt.spawn(async { ... })`:
- Task is scheduled on a runtime worker thread
- Worker threads are initialized with thread-local reactor
- All `tokio::process::Command` calls succeed

### 3. Channel-Based Communication

- `tokio::sync::mpsc::unbounded_channel()` doesn't need a reactor
- Can be created anywhere
- Sender can send from runtime worker threads
- Receiver can receive from GPUI threads
- Perfect bridge between isolated runtime and GPUI

### 4. Clean Separation of Concerns

```
std::thread + Runtime = Docker Operations (needs reactor)
          ↓ (channel)
    GPUI cx.spawn = UI Updates (no reactor needed)
```

---

## Common Pitfalls

### ❌ Don't: Use block_on for reactor operations

```rust
std::thread::spawn(|| {
    let rt = Runtime::new().unwrap();

    // This runs on std::thread, NOT worker thread
    rt.block_on(async {
        Command::new("docker").spawn() // PANIC!
    })
});
```

**Why:** `block_on()` executes the future on the **current thread** (your std::thread), which has no reactor.

### ❌ Don't: Assume enter() gives you reactor

```rust
std::thread::spawn(|| {
    let rt = Runtime::new().unwrap();
    let _guard = rt.enter(); // Sets thread-local runtime

    // Still no reactor on this thread!
    rt.block_on(async {
        Command::new("docker").spawn() // STILL PANICS!
    })
});
```

**Why:** `enter()` just sets the thread-local "current runtime" reference. It doesn't install a reactor on the current thread.

### ✅ Do: Use spawn to get worker thread

```rust
std::thread::spawn(|| {
    let rt = Runtime::new().unwrap();

    // Spawn on worker thread
    let handle = rt.spawn(async {
        Command::new("docker").spawn() // ✅ Works!
    });

    // Block on handle (not the future itself)
    rt.block_on(handle)
});
```

**Why:** `spawn()` schedules work on a worker thread that **has** a reactor.

---

## Performance Considerations

### Thread Overhead

Creating a new `std::thread` and Tokio runtime for each TB2 run has overhead:
- ~1-2ms to spawn thread
- ~5-10ms to create runtime
- ~100KB-1MB memory per runtime

**Acceptable because:**
- TB2 runs take 30s-15min (overhead is <0.01%)
- Only 1-2 concurrent runs typical
- Clean shutdown when std::thread exits

### Alternative: Thread Pool

For higher-throughput scenarios, consider:

```rust
// One-time setup
static TB2_RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Runtime::new().unwrap()
});

// Per-run
let handle = TB2_RUNTIME.spawn(async {
    // Docker work
});
```

**Trade-offs:**
- ✅ No per-run runtime creation cost
- ✅ Lower memory footprint
- ❌ More complex lifetime management
- ❌ Potential for runtime to be poisoned by panics

---

## Testing

### Verify Reactor Availability

```rust
#[tokio::test]
async fn test_has_reactor() {
    // This test runs on Tokio runtime, so it has reactor
    let output = tokio::process::Command::new("echo")
        .arg("hello")
        .output()
        .await
        .unwrap();

    assert!(output.status.success());
}
```

### Test Without Reactor

```rust
#[test]
#[should_panic(expected = "no reactor running")]
fn test_no_reactor() {
    // Regular #[test] has no Tokio runtime
    std::thread::spawn(|| {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            // This will panic
            tokio::process::Command::new("echo")
                .spawn()
                .unwrap();
        });
    }).join().unwrap();
}
```

---

## Debugging Tips

### Enable Tokio Debug Logs

```bash
RUST_LOG=tokio=debug cargo run
```

Look for:
```
tokio::runtime: worker thread started
tokio::process: spawning child process
```

### Check Thread Names

```rust
rt.spawn(async move {
    eprintln!("Thread: {:?}", std::thread::current().name());
    // Should print: Thread: Some("tokio-runtime-worker")
});
```

### Verify Reactor Presence

```rust
if tokio::runtime::Handle::try_current().is_ok() {
    println!("Has runtime handle");
} else {
    println!("No runtime handle");
}
```

**Note:** Having a runtime handle doesn't guarantee reactor access! You must be on a worker thread.

---

## Summary

The TB2 reactor crash was caused by Docker operations (`tokio::process::Command`) being called from GPUI's executor context, which lacks a Tokio reactor.

**Solution:** Move ALL Docker operations (both `run_claude()` and `run_tests()`) into a single `Runtime::spawn()` call that executes on worker threads with full reactor access. Communicate results back to GPUI via channels.

**Key Insight:** `block_on()` runs on the current thread (no reactor), `spawn()` runs on worker threads (has reactor).

**Architecture:** Isolated runtime in std::thread handles Docker work, GPUI handles UI updates, channels bridge the gap.
