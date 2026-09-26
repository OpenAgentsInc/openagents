---
id: tool.background-service-lifecycle
version: 1
kind: tool
title: Start a long-running service so it outlives your shell, and prove it is ready
summary: >-
  A server started from an agent's shell often dies with that shell or blocks
  it. Detach it in its own session with redirected output, record its PID,
  bind the address the client will use, poll a real request until it succeeds
  or times out, and check its log for errors.
tags: [services, daemon, process-management, readiness, http, linux]
applies_when: >-
  A task requires a server, API, model endpoint, or worker to keep running
  after your commands finish, and a grader will connect to it.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
    - hf-model-inference
    - kv-store-grpc
  cites:
    - "W. Richard Stevens and Stephen Rago, Advanced Programming in the UNIX Environment, 3rd ed. (Addison-Wesley, 2013), chapter 9 (sessions, process groups, SIGHUP) and chapter 13 (daemon processes)"
    - "Linux man-pages: setsid(1), nohup(1), setsid(2)"
    - "Python Software Foundation, subprocess.Popen (start_new_session, stdin, stdout, stderr)"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

**Why services disappear.** A process started in the foreground holds the
tool call open; one started with `&` stays in the shell's session and process
group, so it can be killed with the shell (SIGHUP, or the harness killing the
group) and it may still hold the shell's stdout pipe, which keeps the call
from returning. Detach fully:

```sh
setsid nohup ./server --host 0.0.0.0 --port 8000 > /tmp/server.log 2>&1 < /dev/null &
echo $! > /tmp/server.pid
```

In Python: `subprocess.Popen(cmd, stdin=subprocess.DEVNULL,
stdout=logfile, stderr=subprocess.STDOUT, start_new_session=True)`. Where the
environment has a service manager or a documented start script, use that
instead, since that is what the grader's environment will run.

**Addresses.** Bind `0.0.0.0` (or the specific interface) when clients come
from another container or host; `127.0.0.1` only accepts local clients. Use
exactly the port, path prefix, and protocol the task names. If the port is
already in use, find the stale process (`ss -ltnp`) rather than choosing
another port.

**Readiness.** Model servers and apps with migrations can take tens of
seconds to load. Poll with a bounded loop that performs a real request (not
just a TCP connect) until it succeeds, and fail with the log tail if the
deadline passes:

```sh
for i in $(seq 1 60); do curl -fsS http://127.0.0.1:8000/health && break; sleep 1; done
```

## How to check

From a new shell, confirm the process is alive (`kill -0 $(cat /tmp/server.pid)`),
send one real request for each required endpoint and method, including an
error case the task specifies, and read the log for tracebacks or warnings.
Close the shell that started it and repeat the request to confirm the
service survives.
