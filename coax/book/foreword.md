# Foreword

Coax began with the gap between fast runtime code and a slow development loop.
The Verse's native engine moved through Rust and C++; its current name is
Metareal. Native Metal, explicit data layout and a versioned C boundary remain
useful foundations while Coax takes on one checked subsystem at a time.

The goal is broader than the engine. OpenAgents' web, mobile, desktop, agents,
cloud and Verse surfaces share one public protocol and target Coax for product
logic. That contract lives in root `proto/`; language metadata and generated
code must implement it without inventing a second wire schema.

The current compiler is deliberately small. It checks scalar procedures,
executes bounded compile-time computation and emits inspectable C11. Structs,
reflection, jobs, shaders, build plugins and safe reload are design work with
specific consumer and conformance gates, not features that already ship.

This book distinguishes running examples from planned syntax. The compiler,
grammar, protocol, fixtures and book should grow together. A feature earns its
place by improving a real workflow while preserving data, authority and
platform boundaries.

*OpenAgents, October 2026*
