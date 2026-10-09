// WebMCP: the site's tools for an agent working in this browser tab
// (https://webmachinelearning.github.io/webmcp/). The read tools call the
// site's public docs MCP server (/mcp/docs); start_chat fills and sends the
// composer on pages that have one. Without a model context it does nothing.
(function () {
  "use strict";
  var context = document.modelContext || navigator.modelContext;
  if (!context || typeof context.registerTool !== "function") return;

  var id = 0;
  function call(name, args) {
    id += 1;
    return fetch("/mcp/docs", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json, text/event-stream",
        "MCP-Protocol-Version": "2025-06-18"
      },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: id,
        method: "tools/call",
        params: { name: name, arguments: args || {} }
      })
    })
      .then(function (response) { return response.json(); })
      .then(function (reply) {
        if (reply.error) throw new Error(reply.error.message);
        return reply.result;
      });
  }

  function remote(name, description, properties, required) {
    return {
      name: name,
      description: description,
      inputSchema: {
        type: "object",
        properties: properties,
        required: required,
        additionalProperties: false
      },
      annotations: { readOnlyHint: true },
      execute: function (args) { return call(name, args); }
    };
  }

  var tools = [
    remote("list_docs", "List every OpenAgents guide and API guide with its name and summary.", {}, []),
    remote(
      "search_docs",
      "Search the OpenAgents docs for a word or phrase.",
      {
        query: { type: "string", description: "Words to find in the docs." },
        limit: { type: "integer", minimum: 1, maximum: 20, description: "Most results to return." }
      },
      ["query"]
    ),
    remote(
      "read_doc",
      "Read one OpenAgents guide as Markdown, by the name list_docs gives (such as chat or api/quickstart).",
      { name: { type: "string", description: "The guide's name." } },
      ["name"]
    ),
    remote("list_models", "List the OpenAgents API's models and their prices.", {}, [])
  ];

  var form = document.getElementById("chat-form");
  var input = document.getElementById("chat-input");
  if (form && input && typeof form.requestSubmit === "function") {
    tools.push({
      name: "start_chat",
      description: "Send a message to OpenAgents from this page's chat box.",
      inputSchema: {
        type: "object",
        properties: { message: { type: "string", description: "The message to send." } },
        required: ["message"],
        additionalProperties: false
      },
      execute: function (args) {
        input.value = String(args.message || "");
        input.dispatchEvent(new Event("input", { bubbles: true }));
        form.requestSubmit();
        return { content: [{ type: "text", text: "Sent." }] };
      }
    });
  }

  tools.forEach(function (tool) {
    try {
      var done = context.registerTool(tool);
      if (done && typeof done.catch === "function") done.catch(function () {});
    } catch (error) {
      // A browser with an older API shape: skip this tool.
    }
  });
})();
