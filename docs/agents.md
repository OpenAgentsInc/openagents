---
title: Agents
---

# Agents API

Developers can use the Agents API to interact with agents.

## Create agent

POST https://openagents.com/api/v1/agents

<x-markdown>
```shell
curl https://openagents.com/api/v1/agents \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "Data Visualizer",
    "description": "You analyze .csv files and create data visualizations.",
  }'
```
</x-markdown>

## Uploading files

POST https://openagents.com/api/v1/files


## Adding files to agents
POST https://openagents.com/api/v1/agents/{agent_id}/files

## Creating threads
POST https://openagents.com/api/v1/threads

## Adding a message to a thread
POST https://openagents.com/api/v1/threads/{thread_id}/messages
