// The OpenAI JavaScript SDK against our Chat Completions and Responses
// surfaces (#11068): plain, streamed, and tool calls, with only the base URL
// and key changed. Needs the `openai` package where Node resolves it:
//
//   npm install --prefix /tmp/oa-sdk openai
//   cp scripts/dev/openai-sdk-conformance.mjs /tmp/oa-sdk/
//   OPENAGENTS_API_KEY=oak_... node /tmp/oa-sdk/openai-sdk-conformance.mjs \
//       http://127.0.0.1:8790/v1 google/gemini-3.8-flash
//
// The key is read from the environment and never printed. Prints PASS or
// FAIL per check; exits with the number of failures.
import OpenAI from "openai";

const [baseURL, model = "google/gemini-3.8-flash"] = process.argv.slice(2);
if (!baseURL || !process.env.OPENAGENTS_API_KEY) {
  console.error("usage: OPENAGENTS_API_KEY=... node openai-sdk-conformance.mjs BASE_URL [MODEL]");
  process.exit(2);
}
const client = new OpenAI({ baseURL, apiKey: process.env.OPENAGENTS_API_KEY, maxRetries: 0 });
const weather = {
  type: "function",
  function: {
    name: "get_weather",
    description: "The weather in a city.",
    parameters: { type: "object", properties: { city: { type: "string" } }, required: ["city"] },
  },
};
let failures = 0;
async function check(name, fn) {
  try {
    const detail = await fn();
    console.log(`PASS  ${name}  ${detail ?? ""}`);
  } catch (error) {
    failures += 1;
    console.log(`FAIL  ${name}  ${error?.status ?? ""} ${error?.message ?? error}`);
  }
}
const assert = (ok, what) => {
  if (!ok) throw new Error(what);
};

await check("chat.completions.create", async () => {
  const reply = await client.chat.completions.create({
    model,
    messages: [
      { role: "system", content: "Answer with one word." },
      { role: "user", content: "Say hello." },
    ],
    max_tokens: 200,
  });
  assert(reply.choices[0].message.content, "no content");
  assert(reply.choices[0].finish_reason === "stop", reply.choices[0].finish_reason);
  assert(reply.usage?.completion_tokens > 0, "no usage");
  return JSON.stringify(reply.choices[0].message.content);
});

await check("chat.completions stream + include_usage", async () => {
  const stream = await client.chat.completions.create({
    model,
    messages: [{ role: "user", content: "Count to three." }],
    max_tokens: 200,
    stream: true,
    stream_options: { include_usage: true },
  });
  let text = "";
  let finish = null;
  let usage = null;
  for await (const chunk of stream) {
    if (chunk.choices[0]) {
      text += chunk.choices[0].delta.content ?? "";
      finish = chunk.choices[0].finish_reason ?? finish;
    }
    usage = chunk.usage ?? usage;
  }
  assert(text, "no text");
  assert(finish === "stop", finish);
  assert(usage?.completion_tokens > 0, "no usage chunk");
  return JSON.stringify(text);
});

await check("chat.completions tools, streamed", async () => {
  const stream = await client.chat.completions.create({
    model,
    messages: [{ role: "user", content: "Weather in Paris? Use the tool." }],
    tools: [weather],
    tool_choice: "required",
    max_tokens: 300,
    stream: true,
  });
  let name = "";
  let args = "";
  let finish = null;
  for await (const chunk of stream) {
    for (const call of chunk.choices[0]?.delta.tool_calls ?? []) {
      name += call.function?.name ?? "";
      args += call.function?.arguments ?? "";
    }
    finish = chunk.choices[0]?.finish_reason ?? finish;
  }
  assert(name === "get_weather", name);
  JSON.parse(args);
  assert(finish === "tool_calls", finish);
  return `${name}(${args})`;
});

await check("responses.create", async () => {
  const response = await client.responses.create({
    model,
    instructions: "Answer with one word.",
    input: "Name a fruit.",
    max_output_tokens: 200,
  });
  assert(response.status === "completed", response.status);
  assert(response.output_text, "no output_text");
  return JSON.stringify(response.output_text);
});

await check("responses.create stream", async () => {
  const stream = await client.responses.create({ model, input: "Count to three.", stream: true });
  let text = "";
  let done = null;
  for await (const event of stream) {
    if (event.type === "response.output_text.delta") text += event.delta;
    if (event.type === "response.completed") done = event.response.status;
  }
  assert(text, "no text");
  assert(done === "completed", done);
  return JSON.stringify(text);
});

await check("responses.create function tool", async () => {
  const response = await client.responses.create({
    model,
    input: "Weather in Oslo? Use the tool.",
    tools: [{ type: "function", ...weather.function }],
    tool_choice: "required",
  });
  const call = response.output.find((item) => item.type === "function_call");
  assert(call?.name === "get_weather", JSON.stringify(response.output));
  JSON.parse(call.arguments);
  return `${call.name}(${call.arguments})`;
});

await check("models.list", async () => {
  const ids = [];
  for await (const entry of client.models.list()) ids.push(entry.id);
  assert(ids.includes(model), ids.join(","));
  return `${ids.length} models`;
});

console.log(`\n${failures} failed (openai-node ${OpenAI.VERSION ?? ""})`);
process.exit(failures);
