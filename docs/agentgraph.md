---
title: AgentGraph
---

# AgentGraph

AgentGraph is a visual scripting language for AI agent workflows.

<livewire:graph />

## Implementation

Graph, Node, Edge are all Livewire components.
Graph component currently holds the state of Nodes (id, x, y, height, width, title)
The behavior I need is: When the Livewire component is dragged on client, it needs to be sync'd with its edges via some 
shared variable, but only notify the server once the drag is complete.

