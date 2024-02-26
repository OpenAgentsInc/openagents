---
title: Create Agent
curl: >
  curl https://openagents.com/api/v1/agents \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "Data Visualizer",
    "description": "Analyzes .csv files and creates data visualizations.",
    "instructions": "Upload a .csv file to begin.",
    "welcome_message": "Welcome to Data Visualizer! Please upload a .csv file."
  }'
responses:
  - '200': '{"success": true, "message": "successful response!!!"}'
  - '400': '{"error": "asdfsadfsd!!!"}'
---

# Create agent

POST https://openagents.com/api/v1/agents

### Request parameters
* name (string, required): The name of the agent.
* description (string, required): A brief description of the agent's purpose.
* instructions (string, required): Detailed instructions on how the agent operates.
* welcome_message (string, required): A message that users will see when they start interacting with the agent.

### Request example

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

### Response parameters
* success: A boolean indicating whether the operation was successful.
* message: A human-readable message indicating the result of the operation.
* data: An object containing:
  * agent_id: The newly created agent's ID

### Response example

```shell
{
  "success": true,
  "message": "Agent created successfully.",
  "data": {
    "agent_id": 42
  }
}
```
