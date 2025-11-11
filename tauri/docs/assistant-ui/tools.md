# Tools in assistant-ui

## Overview

Tools extend LLM capabilities by allowing models to call functions, fetch data, perform calculations, and interact with external systems. assistant-ui provides comprehensive support for tools with multiple integration patterns.

## Current Status in OpenAgents

**⚠️ Tools are NOT currently working in our implementation.**

### What We Have

- Basic chat streaming with Ollama using `useLocalRuntime`
- Calculator tool defined in `src/tools/calculator.tsx` (not connected)
- textStream-based adapter for simple text responses

### What's Missing

- Tool calls are not being sent to or recognized by the Ollama model
- No tool execution or result handling
- No tool UI rendering

## Tool Integration Patterns

### 1. useLocalRuntime (Current Approach)

We're using `useLocalRuntime` with a custom `ChatModelAdapter`:

```typescript
const adapter: ChatModelAdapter = {
  async *run({ messages, abortSignal }) {
    // Streams text from Ollama
  },
};

const runtime = useLocalRuntime(adapter);
```

**Limitations**:
- ChatModelAdapter's `run` method doesn't receive `tools` parameter
- No built-in way to pass registered tools to the model
- We must manually bridge between assistant-ui tools and model tools

### 2. useChatRuntime (Alternative - Not Implemented)

The Vercel AI SDK provides `useChatRuntime` which better supports tools:

```typescript
import { useChatRuntime } from "@assistant-ui/react-ai-sdk";

// Backend API route handles tools
const runtime = useChatRuntime({
  api: "/api/chat",
});
```

**Benefits**:
- Automatic tool serialization with `frontendTools()`
- Backend can execute tools server-side
- Full streaming support for tool calls and results

**Trade-offs**:
- Requires backend API route
- Can't use Ollama directly from browser (would need proxy)

## Tool Definition Methods

### Method 1: makeAssistantTool (assistant-ui)

For client-side tool execution with assistant-ui:

```typescript
import { makeAssistantTool } from "@assistant-ui/react";
import { z } from "zod";

export const CalculatorTool = makeAssistantTool({
  toolName: "calculator",
  description: "Perform mathematical calculations",
  parameters: z.object({
    operation: z.enum(["add", "subtract", "multiply", "divide"]),
    a: z.number(),
    b: z.number(),
  }),
  execute: async ({ operation, a, b }) => {
    // Tool logic
    return { result: /* ... */ };
  },
});

// Register in app
<AssistantRuntimeProvider runtime={runtime}>
  <CalculatorTool />
</AssistantRuntimeProvider>
```

**Use case**: Client-side tools with useLocalRuntime or useChatRuntime

### Method 2: AI SDK tool() (Vercel AI)

For defining tools in AI SDK format:

```typescript
import { tool } from "ai";
import { z } from "zod";

const calculatorTool = tool({
  description: "Perform calculations",
  parameters: z.object({
    operation: z.enum(["add", "subtract", "multiply", "divide"]),
    a: z.number(),
    b: z.number(),
  }),
  execute: async ({ operation, a, b }) => {
    // Tool logic
  },
});

// Pass to streamText
streamText({
  model: ollama("glm-4.6:cloud"),
  tools: {
    calculator: calculatorTool,
  },
});
```

**Use case**: When using AI SDK's streamText/generateText directly

### Method 3: makeAssistantToolUI (UI-only)

For adding UI to tools defined elsewhere:

```typescript
import { makeAssistantToolUI } from "@assistant-ui/react";

const CalculatorUI = makeAssistantToolUI<
  { operation: string; a: number; b: number }, // Args type
  { result: number; expression: string } // Result type
>({
  toolName: "calculator",
  render: ({ args, result }) => {
    return (
      <div>
        <p>Operation: {args.operation}</p>
        {result && <p>Result: {result.expression}</p>}
      </div>
    );
  },
});
```

**Use case**: Adding custom UI for backend-defined tools

## Implementing Tools with useLocalRuntime + Ollama

### The Challenge

With `useLocalRuntime`, we need to:

1. Register tools with assistant-ui (using `makeAssistantTool`)
2. Convert tools to AI SDK format for Ollama
3. Pass tools to `streamText`
4. Handle tool-call chunks from fullStream
5. Properly yield tool-call/tool-result content blocks

### Attempted Implementation (TypeScript Errors)

We tried implementing full tool support but encountered type errors:

```typescript
// Issues:
// 1. tool() function signature didn't match our usage
// 2. fullStream chunk types have different property names:
//    - textDelta -> text
//    - args -> input
//    - result -> output
// 3. Tool-result content blocks need args and argsText (not just result)
// 4. ChatModelAdapter typing is strict about content block shapes
```

### Working Solution (TODO)

To properly implement tools, we need to:

1. **Define tools in both formats**:
   - AI SDK tool for Ollama
   - makeAssistantTool for UI

2. **Switch to fullStream**:
   ```typescript
   const stream = result.fullStream;
   ```

3. **Handle all chunk types**:
   ```typescript
   switch (chunk.type) {
     case "text-delta":
       // Accumulate text
     case "tool-call":
       // Yield tool-call content block
     case "tool-result":
       // Yield tool-result content block
     case "finish":
       // Complete
   }
   ```

4. **Properly shape content blocks** per assistant-ui's `ThreadAssistantMessagePart` types

## Model Requirements

Not all models support function calling. For Ollama:

### Models WITH Function Calling Support

- `llama3.2` ✅
- `llama3.1` ✅
- `qwen2.5` ✅
- `mistral` ✅
- `firefunction-v2` ✅

### Models WITHOUT Function Calling Support

- `glm-4.6:cloud` ❌ (currently using)
- Most older models

**To test tools, switch to a compatible model**:

```typescript
model: ollama("llama3.2"), // or qwen2.5, mistral
```

## Tool Execution Flow

### With useLocalRuntime

```
1. User sends message
2. Adapter calls streamText with tools
3. Model decides to call tool
4. fullStream emits tool-call chunk
5. Adapter yields tool-call content block
6. AI SDK executes tool (tool.execute)
7. fullStream emits tool-result chunk
8. Adapter yields tool-result content block
9. assistant-ui renders tool UI
10. Model continues with result
```

### With useChatRuntime + Backend

```
1. User sends message
2. Frontend sends to API route with frontendTools
3. Backend calls streamText with tools
4. Model calls tool, backend executes
5. Results streamed back to frontend
6. assistant-ui renders tool UI
```

## Tool UI Rendering

assistant-ui automatically renders tool calls. You can customize with:

### Default ToolFallback

```typescript
import { ToolFallback } from "@assistant-ui/react";

// Shows JSON of tool args and results
<Thread.Messages components={{ ToolFallback }} />
```

### Custom Tool UI

```typescript
const CalculatorUI = makeAssistantToolUI({
  toolName: "calculator",
  render: ({ args, result, status }) => {
    if (status === "executing") return <div>Calculating...</div>;
    if (result) return <div>{result.expression}</div>;
    return null;
  },
});

// Register
<AssistantRuntimeProvider runtime={runtime}>
  <CalculatorUI />
</AssistantRuntimeProvider>
```

## Human-in-the-Loop Tools

Tools can pause execution to request user input:

```typescript
const emailTool = makeAssistantTool({
  toolName: "sendEmail",
  execute: async ({ to, subject, body }, { human }) => {
    // Request user confirmation
    const confirmed = await human({
      type: "confirmation",
      details: { to, subject },
    });

    if (!confirmed) {
      return { status: "cancelled" };
    }

    await sendEmail({ to, subject, body });
    return { status: "sent" };
  },
  render: ({ interrupt, resume }) => {
    if (interrupt) {
      return (
        <div>
          <p>Send email to {interrupt.payload.details.to}?</p>
          <button onClick={() => resume(true)}>Confirm</button>
          <button onClick={() => resume(false)}>Cancel</button>
        </div>
      );
    }
    return null;
  },
});
```

## Tool Context

Tools receive execution context:

```typescript
execute: async (args, context) => {
  // context.abortSignal - For cancellation
  // context.toolCallId - Unique identifier
  // context.human - Request user input
}
```

## Best Practices

1. **Clear Descriptions**: Help the model understand when to use each tool
2. **Type Safety**: Use Zod schemas for parameters
3. **Error Handling**: Catch errors and return user-friendly messages
4. **Validation**: Validate inputs and permissions
5. **Loading States**: Provide UI feedback during execution
6. **Testing**: Test tools independently and with full chat flow
7. **Abort Support**: Respect abortSignal for cancellable operations

## Testing Tools

### Manual Testing

1. Start Ollama with compatible model:
   ```bash
   ollama pull llama3.2
   OLLAMA_FLASH_ATTENTION="1" ollama serve
   ```

2. Update model in App.tsx:
   ```typescript
   model: ollama("llama3.2")
   ```

3. Ask model to use tool:
   ```
   "What is 25 times 17?"
   "Calculate 144 divided by 12"
   ```

4. Check browser console for:
   - "Adapter run called with messages"
   - "Chunk: { type: 'tool-call', ... }"
   - Tool execution logs

### Debugging

Enable console logs to see:
- Adapter calls
- Stream chunks
- Tool calls and results

```typescript
console.log("Adapter run called with messages:", messages);
console.log("Chunk:", chunk);
console.log("Tool call detected:", chunk);
```

## Next Steps

To implement tools in OpenAgents:

1. **Switch to compatible model** (llama3.2, qwen2.5, or mistral)
2. **Implement fullStream handling** with proper TypeScript types
3. **Define tools in both formats** (AI SDK + makeAssistantTool)
4. **Test with simple calculator tool** first
5. **Add more tools** (file operations, web search, etc.)
6. **Add custom tool UI** for better visualization

## References

- [assistant-ui Tools Guide](https://www.assistant-ui.com/docs/guides/Tools)
- [Vercel AI SDK Tools](https://sdk.vercel.ai/docs/ai-sdk-core/tools-and-tool-calling)
- [Ollama Function Calling](https://ollama.com/blog/tool-support)
- [assistant-ui API Reference](https://www.assistant-ui.com/docs/reference)

## Example: Complete Tool Implementation

Here's what a complete implementation would look like (currently not working due to TypeScript issues):

```typescript
import { makeAssistantTool } from "@assistant-ui/react";
import { tool } from "ai";
import { z } from "zod";

// 1. Define tool schema
const calculatorSchema = z.object({
  operation: z.enum(["add", "subtract", "multiply", "divide"]),
  a: z.number(),
  b: z.number(),
});

// 2. Define execution logic
const executeCalculator = async ({ operation, a, b }) => {
  let result;
  switch (operation) {
    case "add": result = a + b; break;
    case "subtract": result = a - b; break;
    case "multiply": result = a * b; break;
    case "divide":
      if (b === 0) throw new Error("Cannot divide by zero");
      result = a / b;
      break;
  }
  return { result, expression: `${a} ${operation} ${b} = ${result}` };
};

// 3. AI SDK tool for model
const calculatorAITool = tool({
  description: "Perform mathematical calculations",
  parameters: calculatorSchema,
  execute: executeCalculator,
});

// 4. assistant-ui tool for UI
export const CalculatorTool = makeAssistantTool({
  toolName: "calculator",
  description: "Perform mathematical calculations",
  parameters: calculatorSchema,
  execute: executeCalculator,
  render: ({ args, result }) => {
    if (result) {
      return <div className="tool-result">{result.expression}</div>;
    }
    return <div>Calculating {args.operation}...</div>;
  },
});

// 5. Use in adapter
const adapter: ChatModelAdapter = {
  async *run({ messages, abortSignal }) {
    const result = streamText({
      model: ollama("llama3.2"),
      messages,
      tools: { calculator: calculatorAITool },
      abortSignal,
    });

    const stream = result.fullStream;
    let text = "";

    for await (const chunk of stream) {
      switch (chunk.type) {
        case "text-delta":
          text += chunk.text;
          yield { content: [{ type: "text", text }] };
          break;

        case "tool-call":
          yield {
            content: [{
              type: "tool-call",
              toolCallId: chunk.toolCallId,
              toolName: chunk.toolName,
              args: chunk.input,
              argsText: JSON.stringify(chunk.input),
            }],
          };
          break;

        case "tool-result":
          // Need to preserve args from tool-call
          yield {
            content: [{
              type: "tool-result",
              toolCallId: chunk.toolCallId,
              toolName: chunk.toolName,
              result: chunk.output,
              // TODO: Need to track args from tool-call
            }],
          };
          break;

        case "finish":
          yield {
            content: text ? [{ type: "text", text }] : [],
            status: { type: "complete", reason: "stop" },
          };
          break;
      }
    }
  },
};

// 6. Register in app
<AssistantRuntimeProvider runtime={runtime}>
  <CalculatorTool />
  <Thread />
</AssistantRuntimeProvider>
```

The main blocker is properly typing the content blocks to match assistant-ui's expectations, particularly for tool-result which needs both the result AND the original args.
