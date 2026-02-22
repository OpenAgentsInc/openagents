def canon: to_entries | sort_by(.key) | from_entries;

def map_event:
  if .method == "thread/started" then
    [
      {
        type: "start",
        threadId: (.params.thread_id // ""),
        source: "codex"
      }
    ]
  elif .method == "turn/started" then
    [
      {
        type: "start-step",
        threadId: (.params.thread_id // ""),
        turnId: (.params.turn_id // ""),
        model: (.params.model // "")
      }
    ]
  elif .method == "item/started" and ((.params.item_kind // "") == "agent_message") then
    [
      {
        type: "text-start",
        id: (.params.item_id // ""),
        turnId: (.params.turn_id // "")
      }
    ]
  elif .method == "item/agentMessage/delta" then
    [
      {
        type: "text-delta",
        id: (.params.item_id // ""),
        channel: "assistant",
        delta: (.params.delta // "")
      }
    ]
  elif .method == "item/reasoning/summaryTextDelta" then
    [
      {
        type: "text-delta",
        id: (.params.item_id // ""),
        channel: "reasoning",
        delta: (.params.delta // "")
      }
    ]
  elif .method == "item/started" and ((.params.item_kind // "") == "mcp_tool_call") then
    [
      {
        type: "tool-input",
        toolCallId: (.params.item_id // ""),
        toolName: (.params.item.tool_name // .params.item.toolName // "unknown_tool"),
        input: (.params.item.arguments // .params.item.input // {})
      }
    ]
  elif .method == "item/toolOutput/delta" then
    [
      {
        type: "tool-output",
        toolCallId: (.params.item_id // ""),
        delta: (.params.delta // "")
      }
    ]
  elif .method == "item/completed" and ((.params.item_kind // "") == "mcp_tool_call") then
    [
      {
        type: "tool-output",
        toolCallId: (.params.item_id // ""),
        status: (.params.item_status // "completed")
      }
    ]
  elif .method == "turn/completed" then
    [
      {
        type: "finish-step",
        turnId: (.params.turn_id // ""),
        status: (.params.status // "completed")
      },
      {
        type: "finish",
        status: (.params.status // "completed")
      }
    ]
  elif .method == "turn/failed" or .method == "turn/aborted" or .method == "turn/interrupted" then
    [
      {
        type: "error",
        code: (.method | gsub("/"; "_")),
        message: (.params.message // "turn failure"),
        retryable: (.params.will_retry // false)
      },
      {
        type: "finish",
        status: "error"
      }
    ]
  elif .method == "codex/error" then
    [
      {
        type: "error",
        code: "codex_error",
        message: (.params.message // "codex error"),
        retryable: (.params.will_retry // false)
      },
      {
        type: "finish",
        status: "error"
      }
    ]
  else
    []
  end;

{
  schema: "openagents.webparity.vercel_sse_fixture_output.v1",
  generated_at: $generated_at,
  scenarios: (
    .scenarios
    | map(
        . as $scenario
        | (($scenario.events | map(map_event[]))) as $mapped_events
        | {
            id: $scenario.id,
            description: $scenario.description,
            events: $mapped_events,
            wire: (
              ($mapped_events | map("data: " + ((.|canon) | tojson) + "\\n\\n") | join(""))
              + "data: [DONE]\\n\\n"
            )
          }
      )
  )
}
