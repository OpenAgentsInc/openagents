# Solver Agent

This doc explains the Solver agent.

Solver runs in the OpenAgents Projects dashboard.

Basic data model for OA Projects:

- Teams have Projecst
- Projects have Issues

There is one Solver per issue. It has an id like `solver/{uuid-of-project}`. It is a Cloudflare Durable Object using their [Agents SDK](https://developers.cloudflare.com/agents/api-reference/).
