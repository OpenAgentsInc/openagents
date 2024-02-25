---
title: Agents
---

# Agents API

Interact with agents.

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

## Add file to agent
POST https://openagents.com/api/v1/agents/{agent_id}/files

<x-markdown>
```shell
curl https://openagents.com/api/v1/agents/{agent_id}/files \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H 'Content-Type: multipart/form-data' \
  -F 'file=@/path/to/yourfile.csv;type=text/csv' \
  -F 'description=CSV data file for visualization'
```
</x-markdown>
