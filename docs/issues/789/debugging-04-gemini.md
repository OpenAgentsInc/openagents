Okay, let's debug this. The error `Failed to parse stream string. Invalid code .` is happening on the **client-side** (`useChat.ts`) within the Vercel AI SDK's stream parser (`parseDataStreamPart`).

This error means the client received a line over the SSE connection that it tried to parse as data, but the line **did not start with a valid code prefix followed by a colon** (e.g., `0:`, `3:`, `9:`, `a:`). The "." in the error message likely just indicates the end of the invalid code found, meaning the code part was effectively **empty**.

Looking at the server code (`index.ts`) and logs:

1.  **Initial Ping:** You have `await responseStream.write(":\n\n");` at the start of the `stream` callback. This is a valid SSE comment. While *technically* correct, some parsers (or specific versions/implementations) might be brittle. If this is the *only* thing sent before the stream closes or errors out immediately, the client might misinterpret it or an empty line following it.
2.  **Piping `result.toDataStream()`:** This *should* generate correctly formatted lines (`0:...`, `9:...`, etc.).
3.  **Worker Logs:**
    *   The logs show successful connection to MCP and discovery/mapping of 26 tools. `extractToolDefinitions` seems to be working and returning a record (object).
    *   `Starting streamText with tools...` is logged.
    *   `Starting stream response` is logged.
    *   **Crucially Missing:** There are *no logs* from *inside* the `stream()` callback (like "Entered stream() callback.", "Piping...", "Stream piping complete.") and no logs from `streamText`'s `onError` or `onFinish`.
    *   **Interpretation:** This strongly suggests that an error occurs *very early* in the process, potentially *before* or *immediately as* the `pipe(result.toDataStream())` starts executing, or the stream closes prematurely. The client receives *something* (perhaps just the initial ping or an empty response) almost immediately, which it cannot parse.

**Debugging Steps:**

1.  **Remove the Initial Ping (Most Likely Culprit):**
    Comment out the initial ping line in `index.ts` within the `stream` callback. It's possible the client parser is choking on receiving *only* this comment before any actual data arrives, especially if the stream errors out right away.

    ```typescript
    // Inside app.post('/') stream callback:
    return stream(c, async responseStream => {
        try {
            console.log("📬 Entered stream() callback."); // Add log here
            // Start with a ping to keep connection alive
            // await responseStream.write(":\n\n"); // <-- COMMENT THIS OUT

            console.log(" Piping result.toDataStream()..."); // Add log here
            // Pipe the SDK stream directly to the response
            await responseStream.pipe(result.toDataStream());

            console.log("✅ Stream processing complete"); // Add log here
        } catch (error) {
            console.error("💥 Critical error in stream handling:", error);
             // Make sure error reporting doesn't cause another invalid line
             try {
                 if (!responseStream.writableEnded) { // Check if stream is still open
                     const errorMessage = error instanceof Error ? error.message : String(error);
                     await responseStream.write(`data: 3:${JSON.stringify(`Stream processing failed: ${errorMessage}`)}\n\n`);
                 }
             } catch (writeError) {
                 console.error("‼️ Failed to write error to stream:", writeError);
             }
        } finally {
             console.log("🚪 Exiting stream() callback."); // Add log here
        }
    });
    ```
    **Retest after removing the ping.** This is the highest probability fix.

2.  **Verify Tool Schema Format:**
    The agent summary correctly states the SDK expects `Record<string, ToolDefinition>`. Your *updated* `tools.ts` `extractToolDefinitions` correctly returns this type (an object map), and your *updated* `index.ts` correctly passes this object map. So, this specific point mentioned in the agent summary *seems* correctly implemented now. However, let's explicitly log the final `tools` object being passed to `streamText` just to be 100% sure it's valid JSON Schema.

    ```typescript
    // Inside app.post('/') before streamText:
    let tools: Record<string, ToolDefinition> = {}; // Ensure type is Record
    try {
        tools = extractToolDefinitions();
        const toolNames = Object.keys(tools);
        console.log(`✅ Extracted ${toolNames.length} tools for LLM:`, toolNames.join(', '));
        // --- ADD THIS LOG ---
        console.log('🔧 Tools object being passed to streamText:', JSON.stringify(tools, null, 2));
        // --- END ADD ---
        if (toolNames.length === 0) {
            console.warn("⚠️ No tools were extracted for the LLM. Tool usage might fail.");
        }
    } catch (toolError) {
        console.error("❌ Failed to extract tools:", toolError);
    }

    console.log("🎬 Starting streamText with tools...");

    const result = streamText({
        model: workersai("@cf/meta/llama-3.3-70b-instruct-fp8-fast"),
        messages,
        // Ensure the type is correct here too
        tools: Object.keys(tools).length > 0 ? tools as Record<string, ToolDefinition> : undefined,
        toolChoice: Object.keys(tools).length > 0 ? 'auto' : undefined,
        // ... onError callback
    });
    ```
    Check the worker logs for this output. Ensure the `parameters` structure within each tool looks like valid JSON Schema (`type: 'object'`, `properties: {...}`, `required: [...]`). The simplified mapping in your current `tools.ts` might be creating invalid schemas that cause `streamText` to fail silently or immediately.

3.  **Check `streamText` Internal Error:**
    The `onError` you added to `streamText` is good, but it might not catch *all* errors, especially very early setup errors. Add more logging around the `streamText` call itself.

    ```typescript
    console.log("🎬 Starting streamText with tools...");
    let result;
    try {
        result = streamText({
            // ... config ...
            onError: (error: Error) => { // Use Error type
               console.error("💥 streamText reported error:", error);
            },
            // Add onFinish for more info
             onFinish: (event) => {
                 console.log(`🏁 streamText finished. Reason: ${event.finishReason}`, event);
             }
        });
        console.log("✅ streamText call succeeded initially."); // Log success if no immediate exception
    } catch (streamTextError) {
        console.error("🚨 streamText call threw immediate exception:", streamTextError);
        // Handle the error - maybe return a JSON error response instead of streaming
        return c.json({ error: "Failed to initialize AI stream" }, 500);
    }

    // ... rest of the streaming logic using 'result'
    ```

4.  **Simplify Temporarily:**
    If the above doesn't reveal the issue, try removing tools completely to see if basic streaming works:

    ```typescript
    const result = streamText({
        model: workersai("@cf/meta/llama-3.3-70b-instruct-fp8-fast"),
        messages,
        tools: undefined, // REMOVE TOOLS
        toolChoice: undefined, // REMOVE TOOL CHOICE
        // ... onError / onFinish ...
    });
    ```
    Does this work and stream back simple text? If yes, the problem is definitely related to the tool definition format or how `streamText` processes them. If no, the problem is more fundamental (e.g., `workers-ai-provider`, model access, core Hono streaming).

**Focus Order:**

1.  **Remove the initial ping (`:\n\n`).** (Highest probability)
2.  Add logging and verify the exact `tools` object structure passed to `streamText`. Ensure it's valid JSON Schema.
3.  Check if `streamText` itself throws an immediate error or reports one via `onError`.
4.  Simplify by removing tools.

The client-side error and the lack of server-side stream logs strongly point to an issue happening right at the beginning of the stream generation on the server.
