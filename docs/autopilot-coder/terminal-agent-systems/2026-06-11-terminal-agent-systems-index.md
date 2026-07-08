# Terminal Agent Systems Index

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This index names the system areas OpenAgents should document before building a
full Bun/Effect terminal coding agent. Each system doc should describe the
user-visible behavior, state model, Effect service boundary, safety rules,
tests, and receipts needed to make the capability production-grade.

## Suggested Doc Template

Each system doc should answer:

- What user-visible capability are we trying to preserve?
- What is the abstract system?
- What is the Bun/Effect boundary: Schema, service, Layer, Stream, Queue,
  Scope, Schedule, persistence, and error model?
- What data crosses trust boundaries?
- What is public, private, local-only, or operator-only?
- What can run unattended?
- What requires user approval?
- What artifacts or receipts prove the work completed?
- What tests, smokes, and failure fixtures are required?
- What OpenAgents issue, roadmap rung, invariant, or product surface owns it?

## P0 Core Runtime And Safety Systems

1. Agent Runtime Kernel
2. Conversation And Query Engine
3. Tool Registry And Tool Contracts
4. File Tool System
5. Shell Execution System
6. Permission And Approval System
7. Sandbox And Workspace Boundary
8. Worktree And Workspace Materialization
9. Task And Background Execution System
10. Plan, Todo, And Progress State
11. Error Taxonomy And Recovery

## P0 Context, Memory, And Model Systems

12. Context Assembly System
13. Compaction And Summarization System
14. Token And Cost Budgeting
15. Model Provider Abstraction
16. Prompt And Instruction Layering
17. Session Memory System
18. Repository Memory And Onboarding
19. Semantic Retrieval And Search
20. LSP And Diagnostics System

## P0 Terminal Product Surface

21. Terminal UI Shell
22. Input And Keybinding System
23. Command System
24. Diff And Patch Review UI
25. Notifications And Attention System
26. Resume, Rewind, And Session Navigation
27. Help, Doctor, And Debug Surfaces

## P1 Extensibility And Integration Systems

28. MCP Client System
29. MCP Server System
30. Plugin System
31. Skill System
32. Hook And Event System
33. Settings And Configuration System
34. Authentication And Credential Storage
35. Git And GitHub Workflow System
36. IDE And Editor Integration
37. Browser And Desktop Integration
38. Voice And Multimodal Input

## P1 Collaboration And Remote Systems

39. Remote Session Bridge
40. Mobile And Web Companion System
41. Team And Shared Memory System
42. Multi-Agent Coordination System
43. External Work Intake System
44. Artifact And Receipt System
45. Scheduling And Cron System

## P1 Operations, Observability, And Release Systems

46. Structured Event Log
47. Telemetry And Privacy System
48. Performance System
49. Update And Release System
50. Migration System
51. Testing And Smoke System
52. Evaluation And Regression System
53. Security Review System
54. Data Retention And Deletion System

## P2 Product Polish Systems

55. Onboarding System
56. Output Style And Persona System
57. Prompt Suggestions And Autocomplete
58. Tips And Education System
59. Theme And Visual Design System
60. Accessibility And Non-Interactive Mode
61. Internationalization And Localization Boundary
62. Enterprise And Managed Policy System

## First Ten System Docs

The first ten system docs define the core safety and capability spine:

1. Agent Runtime Kernel
2. Tool Registry And Tool Contracts
3. Permission And Approval System
4. Shell Execution System
5. File Tool System
6. Worktree And Workspace Materialization
7. Context Assembly System
8. Compaction And Summarization System
9. MCP Client System
10. Task And Background Execution System

These should be kept current with the runtime kernel, tool, permission,
workspace, and task-supervision implementation lanes as they land.
