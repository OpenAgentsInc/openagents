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
  '200':
    description: Successful response
    content:
      application/json:
        schema:
          type: object
          properties:
            success:
              type: boolean
              description: Indicates if the request was successful
              example: true
            message:
              type: string
              description: A message describing the result of the operation
              example: Agent created successfully
            data:
              type: object
              properties:
                agent_id:
                  type: integer
                  description: The ID of the newly created agent
                  example: 42
  '400':
    description: Bad request
    content:
      application/json:
        schema:
          type: object
          properties:
            error:
              type: string
              description: A message describing the error
              example: Invalid request parameters
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
