---
title: Agents
curl: curl blah blah
responses:
  - 200: '{"successful response"}'
  - 400: '{"asdfsadfsd"}'
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

## List agents

GET `https://openagents.com/api/v1/agents`

Returns a list of agents.

### Request parameters

- `limit` (integer, optional): Defaults to 20. A limit on the number of objects to be returned, can range between 1 and 100.
- `order` (string, optional): Defaults to `desc`. Sort order by the `created_at` timestamp of the objects. Use `asc` for ascending order and `desc` for descending order.
- `after` (string, optional): A cursor for use in pagination. `after` is an agent ID that defines your place in the list. For instance, if you make a list request and receive 100 objects, ending with `agent_xyz`, your subsequent call can include `after=agent_xyz` in order to fetch the next page of the list.
- `before` (string, optional): A cursor for use in pagination. `before` is an agent ID that defines your place in the list. For instance, if you make a list request and receive 100 objects, ending with `agent_xyz`, your subsequent call can include `before=agent_xyz` in order to fetch the previous page of the list.

### Request example

```shell
curl "https://openagents.com/api/v1/agents?order=desc&limit=20" \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H 'Content-Type: application/json'
```

### Response example

```json
{
  "object": "list",
  "data": [
    {
      "id": 1,
      "name": "Data Visualizer",
      "description": "Analyzes .csv files and creates data visualizations.",
      "instructions": "Upload a .csv file to begin.",
      "welcome_message": "Welcome to Data Visualizer! Please upload a .csv file.",
      "created_at": "2024-02-25T12:00:00Z"
    },
    {
      "id": 2,
      "name": "Financial Advisor",
      "description": "Provides financial advice based on your spending habits.",
      "instructions": "Link your bank account to begin.",
      "welcome_message": "Welcome to Financial Advisor! Let's optimize your finances.",
      "created_at": "2024-02-24T11:00:00Z"
    },
    {
      "id": 3,
      "name": "Fitness Coach",
      "description": "Designs personalized workout plans.",
      "instructions": "Enter your fitness goals to start.",
      "welcome_message": "Welcome to Fitness Coach! Your journey to fitness begins now.",
      "created_at": "2024-02-23T10:00:00Z"
    }
  ],
  "first_id": 1,
  "last_id": 3,
  "has_more": false
}
```

## Retrieve an agent

GET `https://openagents.com/api/v1/agents/{agent_id}`

Retrieves a single agent.

### Path parameters

- `agent_id` (string, required): The ID of the agent to retrieve.

### Request example

```shell
curl https://openagents.com/api/v1/agents/{agent_id} \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H 'Content-Type: application/json'
```

### Response example
```json
{
  "success": true,
  "data": {
    "id": 42,
    "name": "Data Visualizer",
    "description": "Analyzes .csv files and creates data visualizations.",
    "instructions": "Upload a .csv file to begin.",
    "welcome_message": "Welcome to Data Visualizer! Please upload a .csv file.",
    "created_at": "2024-02-25T12:00:00Z",
    "balance": 0,
    "user_id": 123,
    "published_at": null
  }
}
```


## Add file to agent
POST https://openagents.com/api/v1/agents/{agent_id}/files

### Request parameters
* agent_id (path parameter, required): The unique identifier of the agent to which the file is being added.
* file (file, required): The file to upload.
* description (string, required): A brief description of the file's content or purpose.

### Request example

```shell
curl https://openagents.com/api/v1/agents/{agent_id}/files \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H 'Content-Type: multipart/form-data' \
  -F 'file=@/path/to/yourfile.csv;type=text/csv' \
  -F 'description=CSV data file for visualization'
```

### Response parameters

* success: A boolean indicating whether the operation was successful.
* message: A human-readable message indicating the result of the operation.
* data: An object containing:
  * file_id: The ID of the newly uploaded file.
  * agent_id: The ID of the agent to which the file was added.

### Response example

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
