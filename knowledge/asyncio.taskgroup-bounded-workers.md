---
id: asyncio.taskgroup-bounded-workers
version: 1
kind: method
title: Bound concurrent async jobs with TaskGroup workers
summary: >-
  Use a fixed number of TaskGroup workers sharing a synchronous iterator to
  process awaitable job factories with bounded concurrency, structured failure
  propagation, and awaited cancellation cleanup.
tags: [python, asyncio, concurrency, cancellation]
applies_when: >-
  An asyncio function must execute a finite collection of async callables with
  an upper bound on active work and ensure child tasks are joined when work
  fails or the parent is cancelled.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - cancel-async-tasks
  cites:
    - "Python Software Foundation, Python 3 Library Reference: asyncio — Asynchronous I/O, Task Groups (asyncio.TaskGroup)"
    - "Python Software Foundation, Python 3 Library Reference: asyncio — Runners (asyncio.run)"
    - "Python Software Foundation, Python 3 Library Reference: asyncio Task Cancellation"
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Rather than create one task per input and guard them with a semaphore, create at most `min(limit, number_of_jobs)` worker tasks. Each worker takes the next callable from a shared iterator and awaits it before taking another. Since iterator advancement occurs synchronously between awaits in a single event loop, workers receive distinct jobs without a separate lock. This also avoids creating a potentially huge number of waiting tasks.

Place workers in `asyncio.TaskGroup`: an unhandled worker exception cancels sibling workers, and exiting the group waits for their completion and cleanup before propagating grouped failures. Parent cancellation similarly propagates to workers and waits for their `finally` cleanup. Validate that the concurrency limit is positive; an empty job set naturally creates no workers. Job cleanup belongs in `try/finally`. With `asyncio.run`, the first SIGINT cancels the main task and allows structured cleanup; repeated SIGINT may interrupt cleanup.

This iterator-sharing pattern assumes the collection is finite and that workers consume jobs in the same event loop. It does not make synchronous blocking jobs cooperative, and cancellation safety still depends on jobs not suppressing cancellation indefinitely.

## How to check

```python
import asyncio

async def run_jobs(jobs, limit):
    if limit <= 0:
        raise ValueError("limit must be positive")
    remaining = iter(jobs)

    async def worker():
        for make_job in remaining:
            await make_job()

    async with asyncio.TaskGroup() as group:
        for _ in range(min(limit, len(jobs))):
            group.create_task(worker())
```

Test with instrumented jobs that increment/decrement an active counter in `try/finally`; assert the peak never exceeds the limit and all successful jobs run. Also test that one failing job cancels a sibling, its `finally` executes before the group exits, parent cancellation waits for cleanup, empty input succeeds, and invalid limits are rejected.

Sources: Python Software Foundation, *Python 3 Library Reference*, “asyncio — Asynchronous I/O,” sections “Task Groups” (`asyncio.TaskGroup`) and “Runners” (`asyncio.run`); Python Software Foundation, *Python 3 Library Reference*, “asyncio Task Cancellation.”
