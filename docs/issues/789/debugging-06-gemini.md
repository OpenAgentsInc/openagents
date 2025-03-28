Okay, the new server-side log is the key:

`💥 streamText error: TypeError: Cannot read properties of undefined (reading 'typeName')`

This error is happening *inside* the `streamText` function on the server, specifically reported via its `onError` callback. This strongly indicates that the **input** provided to `streamText` is causing an internal failure within the Vercel AI SDK or the underlying `workers-ai-provider`.

**Analysis:**

1.  **Error Source:** The error `TypeError: Cannot read properties of undefined (reading 'typeName')` typically occurs when code expects an object with certain properties (like a type descriptor) but receives `undefined`. In the context of `streamText` and tools, this almost certainly points to an issue with **how the tool schemas are defined or processed**.
2.  **Tool Schema (`tools.ts`)**: Your `extractToolDefinitions` now returns a `Record<string, ToolDefinition>`. While the *type* is correct for the Vercel AI SDK, the *content* of the generated JSON schemas within the `parameters` field might be invalid or incompatible with what the `workers-ai-provider` or the underlying AI SDK schema processing expects. The simplified logic (special cases + default owner/repo) is the most likely culprit. It might be generating schemas that *look* okay but violate some subtle rule expected by the SDK.
3.  **SDK/Provider Bug:** While less likely, it's *possible* there's a bug in the Vercel AI SDK or the `workers-ai-provider` when processing tool definitions, especially with a large number of tools (26 discovered).
4.  **Server Error -> Client Error:** Because `streamText` fails internally, the `result.toDataStream()` probably yields an empty or immediately closed stream. When Hono tries to pipe this non-stream/empty stream, the client receives either nothing or just headers/empty lines. The client-side Vercel AI SDK parser then tries to read data, finds nothing matching the expected `<code>:<json>\n` format, and throws the `Failed to parse stream string. Invalid code .` error you see in `useChat.ts`.

**Debugging Steps:**

1.  **Simplify `extractToolDefinitions` Drastically (Primary Focus):**
    Let's create the absolute minimal valid tool schema for *all* discovered tools to see if the `TypeError` goes away. This helps isolate whether the *content* of the properties/required fields is the problem.

    Modify `tools.ts` like this:

    ```typescript
    // apps/chatserver/src/mcp/tools.ts

    // ... (keep existing interfaces) ...

    export function extractToolDefinitions(): Record<string, ToolDefinition> {
      const discoveredTools = mcpClientManager.getAllTools();
      console.log(`[extractToolDefinitions] Discovered ${discoveredTools.length} tools from MCP Manager.`);

      if (discoveredTools.length === 0) {
        console.warn("[extractToolDefinitions] No tools discovered. Returning empty object.");
        return {};
      }

      const toolDefinitions: Record<string, ToolDefinition> = {};

      discoveredTools.forEach(mcpTool => {
        console.log(`[extractToolDefinitions] Mapping tool: ${mcpTool.name}`);

        // --- MINIMAL SCHEMA FOR DEBUGGING ---
        // Create the simplest possible valid JSON Schema for parameters:
        // An object type with NO defined properties.
        const minimalParameters: ToolDefinition['parameters'] = {
            type: "object",
            properties: {}, // CRITICAL: Empty properties object
            required: []    // CRITICAL: Empty required array
        };
        // --- END MINIMAL SCHEMA ---

        // Basic validation: Ensure tool name exists
        if (!mcpTool.name) {
            console.warn(`[extractToolDefinitions] Skipping tool with missing name:`, mcpTool);
            return; // Skip this tool
        }

        toolDefinitions[mcpTool.name] = {
          name: mcpTool.name,
          description: mcpTool.description || `Execute the ${mcpTool.name} tool.`, // Ensure description exists
          parameters: minimalParameters // Use the minimal schema
        };
      });

      console.log(`[extractToolDefinitions] Mapped ${Object.keys(toolDefinitions).length} tools with MINIMAL schema`);
      // Log the final structure briefly
      console.log('[extractToolDefinitions] Final minimal tools structure keys:', Object.keys(toolDefinitions).join(', '));

      return toolDefinitions;
    }

    // ... (processToolCall remains the same) ...
    ```

    **Rationale:** This removes *all* complexity from the generated `properties` and `required` fields. If the `TypeError` disappears with this change, we know the problem lies specifically in how the previous version generated those fields (either the special cases or the defaults). If the `TypeError` *persists*, the issue might be deeper (e.g., number of tools, interaction with the provider, SDK bug).

2.  **Retest and Check Server Logs:** Deploy this change and run the chat again. Carefully observe the server logs:
    *   Does the `💥 streamText error: TypeError: Cannot read properties of undefined (reading 'typeName')` **disappear**?
    *   If it disappears, does the client-side error also disappear or change? (You might get different errors later if the LLM tries to use a tool without parameters, but the initial parsing error should be gone).
    *   Do you now see logs from *inside* the `stream()` callback in `index.ts` (like "Piping result.toDataStream()...", "Stream processing complete")?

3.  **If `TypeError` Persists:**
    *   Try reducing the number of tools passed. In `extractToolDefinitions`, after building the `toolDefinitions` object, slice it to include only the first 5 or so tools before returning. Does this change anything?
    *   Double-check the versions of `@ai-sdk/react`, `ai`, and `workers-ai-provider`. Are they compatible?

Let's start with step 1 (simplifying the schema generation drastically). This is the most targeted fix based on the `TypeError`.
