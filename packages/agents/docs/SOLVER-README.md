# Solver Agent

This doc explains the Solver agent.

Solver runs in the OpenAgents Projects dashboard.

Basic data model for OA Projects:

- Teams have Projects
- Projects have Issues

Each Solver is a Cloudflare Durable Object using their [Agents SDK](https://developers.cloudflare.com/agents/api-reference/).

There is one Solver per issue. It has an id like `solver/{uuid-of-issue}`.

Each issue page (at `/issues/{uuid-of-issue}`) features a chat with that agent, where you can see all its actions in a chat history with tools, with a right sidebar showing metadata like connection status and issue details.

Messaging the agent through the issue chatbox appends a UIMessage (from AI SDK) to the agent's `messages` array

Relevant files:

- `packages/agents/src/agents/solver/index.ts`
  - Defines main Solver agent class
- `packages/agents/src/common/open-agent.ts`
  - Defines main OpenAgent class which agents like Solver extend
- `packages/agents/src/agents/solver/prompts.ts`
  - Defines function to generate system prompt including relevant issue/project/team data
- `packages/agents/src/agents/solver/types.ts`
  - `SolverState` has the state of the agent with messages, current issue/project/team, issue comments, implementation steps. It extends BaseAgentState
- `packages/agents/src/agents/solver/tools.ts`
  - `Defines tools specific to Solver`
- `packages/agents/src/common/types.ts`
  - `BaseAgentState` has state common to all Cloudflare OpenAgents (for now Solver and Coder)
- `apps/website/app/routes/issues/$id.tsx`
  - Issue page, includes SolverConnector component
- `apps/website/app/components/agent/solver-connector.tsx`
  - Has the chat
- `apps/website/app/components/agent/solver-controls.tsx`
  - Controls for Solver showing in issue page sidebar
- `packages/agents/docs/shared-inference-implementation.md`
  - Explains inference method used by the agents (may not be fully up to date)
