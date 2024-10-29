# Chat Implementation Refactoring Analysis

## Current Implementation vs Example Implementation

### Event Types
Both implementations use similar event types:
- `0`: Text delta
- `9`: Tool call
- `a`: Tool result

However, the example implementation also includes:
- `d`: Finish reason event (missing in our implementation)

### Initial Response Structure
Example implementation's first response includes all of:
```
0:" branch."
9:{toolCallId, toolName, args}
d:{finishReason:"tool-calls", usage:{promptTokens, completionTokens}}
a:{toolCallId, result}
```

Our implementation only includes:
```
0: Text deltas
9: Tool call
a: Tool result
```

### Key Differences

1. **Missing Finish Events**
   - Example sends `d` events to indicate completion with reason and token usage
   - Our implementation lacks finish events entirely
   - This affects client's ability to know when a response or tool call is complete

2. **Initial Response Structure**
   - Example sends a complete sequence (0,9,d,a) in first response
   - Our implementation sends events more sporadically
   - May affect client's state management and UI updates

3. **Token Usage Tracking**
   - Example includes token usage in finish events
   - Our implementation stores token counts but doesn't stream them

### Code Location for Changes

Primary changes needed in:
1. `app/Traits/UsesChat.php`:
   - `handleToolInvocations()` method needs to emit finish events
   - `createChatCallback()` needs to structure initial response properly
   - `storeAIResponse()` should prepare token usage for streaming

2. `app/Traits/UsesStreaming.php`:
   - Need to add method for streaming finish events
   - Update streaming format to match example

## Recommended Changes

1. Add Finish Event Streaming:
```php
private function streamFinishEvent($reason, $usage = null) {
    $data = [
        'finishReason' => $reason,
        'usage' => $usage ?? [
            'promptTokens' => $this->response['input_tokens'] ?? 0,
            'completionTokens' => $this->response['output_tokens'] ?? 0
        ]
    ];
    $this->stream('d:' . json_encode($data));
}
```

2. Update Tool Invocation Handling:
```php
private function handleToolInvocations() {
    if (!isset($this->response['toolInvocations'])) {
        $this->streamFinishEvent('stop');
        return;
    }

    $toolInvocations = $this->response['toolInvocations'];
    if (empty($toolInvocations)) {
        $this->streamFinishEvent('stop');
        return;
    }

    $this->streamToolCall($toolInvocations);
    $this->streamFinishEvent('tool-calls');

    foreach ($toolInvocations as $toolInvocation) {
        // ... existing tool handling code ...
        $this->streamToolResult($formattedToolResult);
    }

    $this->streamFinishEvent('stop');
}
```

3. Restructure Initial Response:
```php
private function createChatCallback() {
    $this->response = $this->gateway->inference([...]);
    $this->storeAIResponse();

    return function () {
        // Initial response structure
        $this->stream('0:" "'); // Initial text delta
        if (!empty($this->response['toolInvocations'])) {
            $this->streamToolCall($this->response['toolInvocations']);
            $this->streamFinishEvent('tool-calls');
            $this->streamToolResult($formattedToolResult);
        }

        // Continue with regular streaming
        $content = $this->response['content'];
        $words = explode(' ', $content);
        foreach ($words as $word) {
            $this->stream($word . ' ');
            usleep(50000);
        }

        $this->handleToolInvocations();
    };
}
```

## Benefits of Changes

1. Better Client Compatibility
   - Matches expected Vercel AI SDK event structure
   - Provides clear completion signals

2. Improved State Management
   - Clients can better track tool call states
   - Clear indication of response completion

3. Better Token Usage Tracking
   - Token usage information available to clients
   - Helps with rate limiting and usage monitoring

## Implementation Priority

1. Add finish event streaming functionality
2. Update tool invocation handling
3. Restructure initial response
4. Add token usage tracking to finish events