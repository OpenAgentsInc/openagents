---
title: Threads
---

# Threads API

Manage agent threads.

## Create thread
**POST https://openagents.com/api/v1/threads**

To initiate a new conversation with an agent, send a POST request to create a conversation entry. The agent's ID obtained during agent creation should be included in the request.

## Retrieve messages in a thread
**GET https://openagents.com/api/v1/threads/{thread_id}/messages**

Retrieve all messages within a thread by making a GET request to the thread's message endpoint using the thread ID.

<x-markdown>
```shell
curl https://openagents.com/api/v1/threads/{thread_id}/messages \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY"
```
</x-markdown>
