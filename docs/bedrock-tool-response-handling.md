# Bedrock Tool Response Handling

## Issue
When using tools with Claude on Bedrock, we encountered an issue where the assistant would continue generating text after a tool call, leading to undesired behavior like this:

```
0:" "
9:{"toolCallId":"tooluse_ZTB7hmVITFOA0WoWdqGeCQ","toolName":"view_file","args":{"owner":"openagentsinc","repo":"openagents","path":"README.md","branch":"main"}}
d:{"finishReason":"tool-calls","usage":{"promptTokens":945,"completionTokens":161}}
a:{"toolCallId":"tooluse_ZTB7hmVITFOA0WoWdqGeCQ","result":{"type":"tool_call","value":{"toolCallId":"tooluse_ZTB7hmVITFOA0WoWdqGeCQ","toolName":"view_file","args":{"owner":"openagentsinc","repo":"openagents","path":"README.md","branch":"main"},"result":{"success":false,"error":"Failed to retrieve file from GitHub","details":"Client error: `GET https:\/\/api.github.com\/repos\/openagentsinc\/openagents\/contents\/README.md?ref=main` resulted in a `401 Unauthorized` response:\n{\"message\":\"Bad credentials\",\"documentation_url\":\"https:\/\/docs.github.com\/rest\",\"status\":\"401\"}\n"}}}}
0:"I "
0:"apologize "
0:"for "
0:"the "
```

The assistant would make a tool call, receive the result, and then continue generating an apology or additional text, which was not the desired behavior.

## Solution
We modified the response handling in the BedrockMessageFormatting trait to stop processing text content after encountering a tool use. Here's what we changed:

1. Added a flag to track when a tool use is encountered:
```php
protected function formatResponse(array $decodedBody): array
{
    $response = $this->initializeResponse($decodedBody);
    $foundToolUse = false;  // New flag
    // ...
}
```

2. Modified the content processing loop to stop processing text after a tool use:
```php
foreach ($decodedBody['output']['message']['content'] as $contentItem) {
    // If we've found a tool use, only process tool-related items
    if ($foundToolUse) {
        if (isset($contentItem['toolUse'])) {
            $this->processContentItem($contentItem, $response);
        }
        continue;
    }

    $this->processContentItem($contentItem, $response);
    
    // If this was a tool use, set the flag
    if (isset($contentItem['toolUse'])) {
        $foundToolUse = true;
    }
}
```

3. Added unit tests to verify the behavior:
- `testFormatResponseStopsAfterToolUse`: Ensures text after a tool use is ignored
- `testFormatResponseHandlesMultipleToolUses`: Verifies multiple tool uses work while ignoring intermediate text

## Expected Behavior
Now when the assistant makes a tool call:
1. Any text before the tool call is preserved
2. The tool call itself is processed
3. Any text content after the tool call is ignored
4. Additional tool calls (if any) are still processed
5. The response maintains the correct structure with all necessary information

## Files Modified
- `app/AI/Traits/BedrockMessageFormatting.php`
- `tests/Unit/BedrockMessageFormattingTest.php`

## Testing
You can verify this behavior by:
1. Running the unit tests: `php artisan test --filter=BedrockMessageFormattingTest`
2. Making a tool call in the chat interface and verifying no additional text appears after the tool call

## Related Issues
- Consecutive assistant messages in chat flow
- Tool call response handling
- Response streaming behavior

## Future Considerations
1. Monitor for any edge cases where this behavior might need adjustment
2. Consider adding configuration options for different response handling strategies
3. Keep track of Bedrock API changes that might affect this behavior