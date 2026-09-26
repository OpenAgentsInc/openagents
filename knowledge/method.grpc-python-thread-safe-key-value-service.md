---
id: method.grpc-python-thread-safe-key-value-service
version: 1
kind: method
title: Implement a thread-safe in-memory gRPC key-value service in Python
summary: >-
  Define RPC contracts in Protocol Buffers, generate Python stubs, and
  implement a concurrent server with synchronized shared state and explicit
  missing-key status. Applies to small unary gRPC services backed by
  process-local memory.
tags: [grpc, protobuf, python, concurrency]
applies_when: >-
  A Python gRPC service handles concurrent unary reads and writes against
  shared mutable in-memory state.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - kv-store-grpc
  cites:
    - "gRPC Authors, “gRPC Python: Basics,” sections “The Protocol Buffer Compiler” and “Implementing the server”"
    - Protocol Buffers Authors, “Proto3 Language Guide,” sections “Defining A Service” and “Scalar Value Types”
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Define request and response messages and service methods in a `.proto` file, then generate Python message and gRPC modules with `python -m grpc_tools.protoc -I. --python_out=. --grpc_python_out=. schema.proto`. Import the generated service base class and implement each RPC with the exact generated method name and request/response types. Register the servicer on a `grpc.server` backed by a `concurrent.futures.ThreadPoolExecutor`, bind the endpoint, start, and wait for termination.

A thread-pool server may invoke handlers concurrently. Protect compound access to a shared dictionary with a lock; keep the critical section small, and do not hold the lock while aborting an RPC. Distinguish absence with membership or a `None` sentinel rather than truthiness, so valid stored values such as integer zero remain readable. Report absent keys with an intentional gRPC status such as `NOT_FOUND`. Process-local memory is neither durable nor shared across multiple server processes.

Sources: gRPC Authors, “gRPC Python: Basics,” sections “The Protocol Buffer Compiler” and “Implementing the server”; Protocol Buffers Authors, “Proto3 Language Guide,” sections “Defining A Service” and “Scalar Value Types.”

## How to check

Generate the stubs, then exercise the service through a real local channel. Check set/get, overwrite, zero-valued data, absent keys and the expected RPC status, and concurrent operations. For example:

```python
with grpc.insecure_channel("localhost:50051") as channel:
    grpc.channel_ready_future(channel).result(timeout=5)
    stub = service_pb2_grpc.KeyValueStub(channel)
    stub.Put(PutRequest(key="k", value=0), timeout=5)
    assert stub.Get(GetRequest(key="k"), timeout=5).value == 0
    try:
        stub.Get(GetRequest(key="absent"), timeout=5)
    except grpc.RpcError as exc:
        assert exc.code() == grpc.StatusCode.NOT_FOUND
    else:
        raise AssertionError("missing key should fail")
```
