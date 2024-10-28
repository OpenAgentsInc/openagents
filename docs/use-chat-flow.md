Files with relevant functionality:

- resources/js/Pages/Chat.tsx
- resources/js/components/chat/ChatInput.tsx
- app/Http/Controllers/UseChatController.php
- app/Traits/UsesChat.php
- app/Traits/UsesStreaming.php

Client uses Vercel AI SDK - React

```json
{
  "messages": [
    {
      "id": "1730138843458",
      "role": "user",
      "content": "Test",
      "user": {
        "name": "Christopher David"
      },
      "user_id": 1
    }
  ],
  "thread_id": 1,
  "selected_tools": [
    "view_file",
    "view_folder"
  ]
}
```

The server returns a StreamedResponse in the format expected by the Vercel useChat hook.

Events:
- 0: Text delta
- 9: Tool call
- a: Tool result

Example response from the above request:
```
0:"I "
0:"apologize, "
0:"but "
0:"I "
0:"need "
0:"more "
0:"information "
0:"to "
0:"assist "
0:"you "
0:"effectively. "
```
