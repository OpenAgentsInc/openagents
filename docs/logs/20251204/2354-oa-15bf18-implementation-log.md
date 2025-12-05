# 2354 Work Log
- Implemented memory-aware maxAgents guardrails.
- Added perAgentMemoryMb and hostMemoryReserveMb defaults to ParallelExecutionConfig (4096/6144 MiB).
- Parallel runner now computes a safe maxAgents based on host total memory and logs when caps apply.
- Added unit tests for the safety calculator.
