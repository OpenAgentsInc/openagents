---
title: Overview
---

# API Overview

The OpenAgents API helps developers define and interact with agents programmatically.

## Definitions

* **Agent** - An AI entity executing defined tasks
* **Thread** - A message chain between user and agent
* **Message** - A single communication in a thread
* **File** - Documents processed or created by agents
* **Run** - The active execution of an agent flow
* **Flow** - A sequence of nodes
* **Node** - An individual task within a flow
* **Plugin** - A WebAssembly binary extending agent functionality

## Concepts

Agents are built from interchangeable building blocks called Nodes, which are organized into Flows to specify behavior.

Example Nodes include: API endpoints, conditional logic, data parsing, and third-party integrations.

Nodes can be created by community developers by uploading plugins.

Each Node may have an associated fee, payable to its creator upon use.

Nodes can reference an agent's Files.

Plugins are special type of Node that allow for bespoke operations and enhanced capabilities.

Users converse with Agents in conversations that are Threads of Messages.

A Run is an instance of executing a Flow.
