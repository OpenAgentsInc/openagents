# Solver Agent

This doc explains the Solver agent.

Solver runs in the OpenAgents Projects dashboard.

Basic data model for OA Projects:

- Teams have Projects
- Projects have Issues

Each Solver is a Cloudflare Durable Object using their [Agents SDK](https://developers.cloudflare.com/agents/api-reference/).

There is one Solver per issue. It has an id like `solver/{uuid-of-issue}`.

Each issue page (at `/issues/{uuid-of-issue}`) features a chat with that agent, where you can see all its actions in a chat history with tools, with a right sidebar showing metadata like connection status and issue details.
