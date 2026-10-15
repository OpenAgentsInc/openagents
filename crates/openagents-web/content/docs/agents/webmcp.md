# Use WebMCP tools in a browser agent

Pages on openagents.com hand their tools to the browser's agent through
WebMCP. A browser agent can read the docs, list the models, and start a
chat without reading the page's layout.

## How the page offers them

Each page loads one script:

```html
<script src="/static/webmcp.js" defer></script>
```

It looks for `document.modelContext` (or `navigator.modelContext`) and
calls `registerTool` once for each tool. In a browser with no WebMCP, it
does nothing.

## The tools

| Tool | Input | What it does |
| --- | --- | --- |
| `list_docs` | `{}` | Lists every page with its name and summary |
| `search_docs` | `{"query": "...", "limit": 5}` (`query` required, `limit` 1 to 20) | Searches the docs |
| `read_doc` | `{"name": "agents/start-here"}` | Reads one page as Markdown |
| `list_models` | `{}` | Lists the API's models and their prices |
| `start_chat` | `{"message": "..."}` | Sends a message from the page's chat box |

The first four only read. They call our docs MCP server
(`POST https://openagents.com/mcp/docs`) and return its `result` object.
`start_chat` is offered only on a page with a chat box, such as
[openagents.com](https://openagents.com/): it fills the box, sends it, and
returns `Sent.`; the answer shows on the page.

## Step 1: list what the page offers

In a browser with WebMCP, ask the page's model context for its tools.
From the browser's console, the same tools can be checked by name:

```js
const context = document.modelContext || navigator.modelContext;
console.log(Boolean(context && context.registerTool));
```

## Step 2: call a tool

Your browser agent calls a tool by name with its input. For example, to
read the start page:

```json
{"name": "read_doc", "arguments": {"name": "agents/start-here"}}
```

The result is the MCP result shape:

```json
{"content": [{"type": "text", "text": "# For agents: start here\n..."}], "isError": false}
```

## Step 3: start a chat

On [openagents.com](https://openagents.com/), logged in:

```json
{"name": "start_chat", "arguments": {"message": "I need a summary of my open pull requests."}}
```

The tool returns `Sent.` once the message is sent. Read the answer from
the page.

## Without WebMCP

The same read tools are on the docs MCP server, which any agent can call
over HTTP. See [Connect to our MCP servers](/docs/agents/mcp).
