Files with relevant functionality:

- resources/js/Pages/Chat.tsx
- resources/js/components/chat/ChatInput.tsx
- app/Http/Controllers/UseChatController.php
- app/Traits/UsesChat.php
- app/Traits/UsesStreaming.php
- resources/js/lib/useChat.ts
- app/AI/BedrockAIGateway.php
- tests/Feature/UseChatToolsTest.php
- docs/converse.md

Client uses Vercel AI SDK - React

```json
{
    "messages": [
        {
            "id": "1730139263877",
            "role": "user",
            "content": "Open the README on the openagentsinc/openagents main branch and summarize in 1 sentence.",
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
0:"Certainly! "
0:"I'll "
0:"use "
0:"the "
0:"`view_file` "
0:"function "
0:"to "
0:"open "
0:"the "
0:"README "
0:"file "
0:"from "
0:"the "
0:"main "
0:"branch "
0:"of "
0:"the "
0:"openagentsinc\/openagents "
0:"repository "
0:"and "
0:"then "
0:"summarize "
0:"it "
0:"for "
0:"you "
0:"in "
0:"one "
0:"sentence. "
0:"Here's "
0:"the "
0:"function "
0:"call: "
9:{"toolCallId":"tooluse_iX3vRKRqSEW3MNt34OoE2w","toolName":"view_file","args":{"owner":"openagentsinc","repo":"openagents","path":"README.md","branch":"main"}}
a:{"toolCallId":"tooluse_iX3vRKRqSEW3MNt34OoE2w","result":{"type":"tool_call","value":{"toolCallId":"tooluse_iX3vRKRqSEW3MNt34OoE2w","toolName":"view_file","args":{"owner":"openagentsinc","repo":"openagents","path":"README.md","branch":"main"},"result":{"success":false,"error":"Failed to retrieve file from GitHub","details":"Client error: `GET https:\/\/api.github.com\/repos\/openagentsinc\/openagents\/contents\/README.md?ref=main` resulted in a `401 Unauthorized` response:\n{\"message\":\"Bad credentials\",\"documentation_url\":\"https:\/\/docs.github.com\/rest\",\"status\":\"401\"}\n"}}}}

```
