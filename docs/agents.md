---
title: Agents
---

# Agents API

Interact with agents.

## Create agent

POST https://openagents.com/api/v1/agents

### Request parameters
* name (string, required): The name of the agent.
* description (string, required): A brief description of the agent's purpose.
* instructions (string, required): Detailed instructions on how the agent operates.
* welcome_message (string, required): A message that users will see when they start interacting with the agent.

### Request example

<x-markdown class="mt-6">
```shell
curl https://openagents.com/api/v1/agents \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "Data Visualizer",
    "description": "Analyzes .csv files and creates data visualizations.",
    "instructions": "Upload a .csv file to begin.",
    "welcome_message": "Welcome to Data Visualizer! Please upload a .csv file."
  }'
```
</x-markdown>

### Response parameters
* success: A boolean indicating whether the operation was successful.
* message: A human-readable message indicating the result of the operation.
* data: An object containing:
  * agent_id: The newly created agent's ID

### Response example

<x-markdown class="mt-6">
```shell
{
  "success": true,
  "message": "Agent created successfully.",
  "data": {
    "agent_id": 42
  }
}
```
</x-markdown>

## Add file to agent
POST https://openagents.com/api/v1/agents/{agent_id}/files

### Request parameters
* agent_id (path parameter, required): The unique identifier of the agent to which the file is being added.
* file (file, required): The file to upload.
* description (string, required): A brief description of the file's content or purpose.

### Request example

<x-markdown class="mt-6">
```shell
curl https://openagents.com/api/v1/agents/{agent_id}/files \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H 'Content-Type: multipart/form-data' \
  -F 'file=@/path/to/yourfile.csv;type=text/csv' \
  -F 'description=CSV data file for visualization'
```
</x-markdown>

### Response parameters

* success: A boolean indicating whether the operation was successful.
* message: A human-readable message indicating the result of the operation.
* data: An object containing:
  * file_id: The ID of the newly uploaded file.
  * agent_id: The ID of the agent to which the file was added.

### Response example

<x-markdown class="mt-6">
```json
{
  "success": true,
  "message": "File added to agent successfully.",
  "data": {
    "file_id": 123,
    "agent_id": 42
  }
}
```
</x-markdown>
