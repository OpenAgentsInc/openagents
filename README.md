# OpenAgents

OpenAgents is a platform for building and commercializing AI agents using shared databases of skills and knowledge. It
will soon include a two-sided marketplace for builders and users of AI agents.

See the [wiki](https://github.com/OpenAgentsInc/openagents/wiki) for more.

See the [changelog](https://openagents.com/changelog) for recent changes.

We have a [community](https://stacker.news/~openagents) on Stacker News.

![agentstore](https://github.com/OpenAgentsInc/openagents/assets/14167547/4acf99e7-09ec-4f05-b28a-08b380ad683a)

![builder1](https://github.com/OpenAgentsInc/openagents/assets/14167547/2114cfed-5731-4d50-9a11-1f58de3b41e9)

## How it works

- A user creates an Agent from Nodes.
- Nodes are defined by community developers. They could be:
    - API endpoints
    - External WASM plugins
    - Conditional logic
    - Data parsing
    - Or most anything compatible with flow-based programming
- Nodes may have an associated fee which is paid to the node creator when the node is used in a workflow
- Agents can be used in our UI and via API
- Users can comment/rate/share Nodes
- Leaderboards show what's popular

## Tech Stack

- [TALL](https://tallstack.dev/)
    - Tailwind
    - Alpine
    - Laravel
    - Livewire -> HTMX

## Video series

We've chronicled most of the development of this platform over multiple months and 80+ videos on X.

See [episode one](https://twitter.com/OpenAgentsInc/status/1721942435125715086) or
the [full episode list](https://github.com/OpenAgentsInc/openagents/wiki/Video-Series).
