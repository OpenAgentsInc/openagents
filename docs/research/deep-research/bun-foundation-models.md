> I have a Macbook Pro M2 running iOS 26. How can I access the new Foundation Models API from a Bun app?

Good question. Short answer: it’s not (currently) straightforward to directly call Foundation Models (from Apple’s “Apple Intelligence”) from a Bun app — because Foundation Models is a **native-Apple (Swift/Objective-C/SwiftUI) framework**, not a general HTTP API. ([Apple Developer][1])

Here’s a breakdown of why — and what your (limited) options are:

## ✅ What Foundation Models is — and how it’s intended to be used

* Foundation Models is an on-device LLM framework baked into iOS 26 / macOS 26 etc. for devices that support “Apple Intelligence.” ([Apple][2])
* You use it by importing the `FoundationModels` module in Swift, obtaining a `SystemLanguageModel` (e.g. `SystemLanguageModel.default`), checking its `.availability`, and then creating a `LanguageModelSession` to call `respond(to:)` or `streamResponse(...)`. ([appcoda.com][3])
* The framework supports structured output (via the `@Generable` macro), tool-calling (i.e. letting the model call back into your app’s Swift code), and other advanced features. ([Apple Developer][4])

In other words: Foundation Models is designed for use **inside native Apple apps** (iOS, macOS, visionOS, etc.), not a standalone cloud API.

## 🚫 Why you can’t just “import Foundation Models in Bun”

* Bun is a JavaScript runtime (on macOS, Linux, Windows) using JavaScriptCore under the hood. ([Wikipedia][5])
* Foundation Models is not exposed as a JavaScript/Node-style package or HTTP REST/gRPC endpoint. It’s a Swift (or Obj-C) framework that tightly integrates with Apple platform APIs, macOS runtime, and Apple-Silicon hardware. ([createwithswift.com][6])
* There’s no official “Foundation Models for Node / JavaScriptCore / Bun” bridge published by Apple (as of the sources from 2025), and community-driven Node bindings are anecdotal, unverified, or speculative. For example one post on Reddit claims there are “macOS 26 Foundation Model bindings for Node.js,” but doesn’t link to an official library or documentation, so that route is unreliable for production. ([Reddit][7])

## 🔍 What you *could* do if you really want to use Foundation Models from Bun (or JS)

If you absolutely want to leverage Foundation Models while using Bun (or other JS runtimes), here are possible (but imperfect / work-involved) options:

1. **Write a small native “bridge” helper in Swift (or Objective-C / Swift) — a wrapper app/binary that exposes a local API (e.g. via HTTP, IPC, or similar).**

   * That wrapper app would import FoundationModels, accept prompts (e.g. via HTTP POST), run the prompt through `LanguageModelSession`, and return the generated text/structured output.
   * Your Bun code could then call that local API.
   * Downsides: you have to maintain this Swift helper, handle concurrency, startup time, error cases, and distribution (users must allow / run the helper).

2. **Embed Swift code directly using macOS bridging for JavaScriptCore — but this is very advanced and brittle.**

   * Since Bun uses JavaScriptCore, in principle one could write a macOS “native addon” that links to FoundationModels and exposes JS bindings. But to my knowledge, no publicly distributed library does this yet.

3. **Wait for — or build — community bindings / libraries.**

   * If someone builds a stable, well-maintained JS/Node (or Bun) wrapper around Foundation Models (or Apple offers one), then you could import it directly in Bun.
   * As of late 2025, such a library doesn’t appear to exist in a mature form. The Reddit thread citing “macOS 26 Foundation Model bindings for Node.js” is anecdotal and likely experimental. ([Reddit][7])

4. **Use Foundation Models in a native part of your app — e.g. build a thin Swift service or micro-app that handles LLM logic, and have Bun communicate with it.**

   * If your project is hybrid (e.g. a macOS app with a JS backend), isolating the AI logic in native Swift and bridging via HTTP/IPC may be the most pragmatic approach.

## 🎯 My Recommendation (Given Your Setup)

Since you have a **MacBook Pro M2** running macOS 26 (iOS 26 is probably a slip — iOS is for iPhone; macOS 26 is the OS on Mac), here’s what I recommend if you want to experiment with Foundation Models **while still using Bun**:

* Build a small **native Swift “bridge server”** that wraps Foundation Models and exposes an HTTP API (e.g. `POST /prompt -> { response }`).
* Run that helper locally, and then have your Bun application call it as if it were a backend API.
* Use that only for prototyping or internal tools — for a production app on macOS/iOS, consider writing the main logic in Swift/SwiftUI to better integrate with the framework.

If you like — I can **sketch out a minimal example** of such a wrapper (Swift + Bun) for you — around 30–40 lines of code — as a starting point.
Do you want me to build that skeleton now for you?

[1]: https://developer.apple.com/documentation/FoundationModels?utm_source=chatgpt.com "Foundation Models | Apple Developer Documentation"
[2]: https://www.apple.com/newsroom/2025/09/apples-foundation-models-framework-unlocks-new-intelligent-app-experiences/?utm_source=chatgpt.com "Apple's Foundation Models framework unlocks new ..."
[3]: https://www.appcoda.com/foundation-models/?utm_source=chatgpt.com "Getting Started with Foundation Models in iOS 26"
[4]: https://developer.apple.com/videos/play/wwdc2025/301/?utm_source=chatgpt.com "Deep dive into the Foundation Models framework - WWDC25 ..."
[5]: https://en.wikipedia.org/wiki/Bun_%28software%29?utm_source=chatgpt.com "Bun (software)"
[6]: https://www.createwithswift.com/exploring-the-foundation-models-framework/?utm_source=chatgpt.com "Exploring the Foundation Models framework"
[7]: https://www.reddit.com/r/LocalLLaMA/comments/1le5t5k/macos_26_foundation_model_bindings_for_nodejs/?utm_source=chatgpt.com "MacOS 26 Foundation Model Bindings for Node.js : r ..."


> yes please

Perfect, let’s do a clean little “Swift bridge ⟷ Bun app” setup.

I’ll give you:

1. A **Swift CLI bridge** that:

   * Reads `{ "prompt": "..." }` from stdin (JSON)
   * Calls a placeholder `callFoundationModel(prompt:)`
   * Writes `{ "output": "..." }` to stdout (JSON)

2. A **Bun script** that:

   * Spawns that Swift binary
   * Sends the prompt
   * Reads the response and returns the model output

You then just drop in the real Foundation Models call inside the Swift function.

---

## 1️⃣ Swift bridge (CLI tool)

Create a Swift package or just a single-file CLI target, e.g. `FoundationBridge/main.swift`:

```swift
// main.swift
//
// Simple STDIN/STDOUT JSON bridge to Apple Foundation Models.
// Build with: swiftc main.swift -o foundation-bridge

import Foundation

// MARK: - Types

struct Request: Decodable {
    let prompt: String
}

struct Response: Encodable {
    let output: String
    // add more fields later if you want (usage, tokens, etc.)
}

// MARK: - Foundation Models wrapper (placeholder)

func callFoundationModel(prompt: String) throws -> String {
    // TODO: Replace this with real FoundationModels code.
    //
    // Rough shape (pseudocode):
    //
    // import FoundationModels
    // let model = try SystemLanguageModel.default
    // let session = try model.makeSession()
    // let reply = try session.respond(to: prompt)
    // return reply.text
    //
    // For now we stub it:
    return "Echo from FoundationModels stub: \(prompt)"
}

// MARK: - Main

func readAllStdin() -> Data {
    var data = Data()
    while let chunk = FileHandle.standardInput.availableData.nonEmpty {
        data.append(chunk)
    }
    return data
}

extension Data {
    var nonEmpty: Data? {
        isEmpty ? nil : self
    }
}

do {
    let inputData = readAllStdin()
    let decoder = JSONDecoder()
    let request = try decoder.decode(Request.self, from: inputData)

    let outputText = try callFoundationModel(prompt: request.prompt)

    let response = Response(output: outputText)
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.prettyPrinted]

    let responseData = try encoder.encode(response)
    FileHandle.standardOutput.write(responseData)
    FileHandle.standardOutput.write("\n".data(using: .utf8)!)
} catch {
    // On error, print a JSON error payload (so Bun can see what went wrong)
    let errorDict: [String: String] = [
        "error": "bridge_failed",
        "message": String(describing: error)
    ]
    if let data = try? JSONSerialization.data(withJSONObject: errorDict, options: []) {
        FileHandle.standardOutput.write(data)
        FileHandle.standardOutput.write("\n".data(using: .utf8)!)
    }
    exit(1)
}
```

### Build it

From the directory with `main.swift`:

```bash
swiftc main.swift -o foundation-bridge
```

You’ll get a binary `./foundation-bridge` you can call from Bun.

Later, when you’re ready, you’ll replace `callFoundationModel(prompt:)` with the actual FoundationModels API from Apple’s docs.

---

## 2️⃣ Bun side: call the bridge

Create a Bun script like `bridge-client.ts`:

```ts
// bridge-client.ts
// Bun script that calls the Swift foundation-bridge binary.

type BridgeRequest = { prompt: string };
type BridgeResponse = { output?: string; error?: string; message?: string };

async function callFoundationBridge(prompt: string): Promise<string> {
  const req: BridgeRequest = { prompt };

  const proc = Bun.spawn(
    ["./foundation-bridge"], // path to your compiled Swift binary
    {
      stdin: "pipe",
      stdout: "pipe",
      stderr: "pipe",
    }
  );

  // Send JSON request to stdin
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();

  const stdinJson = JSON.stringify(req) + "\n";
  proc.stdin!.write(encoder.encode(stdinJson));
  proc.stdin!.end();

  // Collect stdout
  const stdoutData = await new Response(proc.stdout).text();
  const stderrData = await new Response(proc.stderr).text();

  if (stderrData.trim().length > 0) {
    console.error("bridge stderr:", stderrData);
  }

  let parsed: BridgeResponse;
  try {
    parsed = JSON.parse(stdoutData);
  } catch (e) {
    throw new Error(
      `Failed to parse bridge response as JSON: ${e}\nRaw: ${stdoutData}`
    );
  }

  if (parsed.error) {
    throw new Error(
      `Bridge returned error: ${parsed.error} - ${parsed.message ?? ""}`
    );
  }

  if (!parsed.output) {
    throw new Error(`Bridge did not return an 'output' field`);
  }

  return parsed.output;
}

// Tiny demo:
const prompt = process.argv.slice(2).join(" ") || "Hello from Bun to FoundationModels!";
callFoundationBridge(prompt)
  .then((out) => {
    console.log("Model output:", out);
  })
  .catch((err) => {
    console.error("Error calling bridge:", err);
    process.exit(1);
  });
```

Run it with:

```bash
bun run bridge-client.ts "Write a haiku about OpenAgents"
```

If you haven’t wired the real model yet, you should see:

```text
Model output: Echo from FoundationModels stub: Write a haiku about OpenAgents
```

---

## 3️⃣ Where you plug in the real Foundation Models code

Inside `callFoundationModel(prompt:)` in Swift, swap the stub for the actual API call, roughly like:

```swift
import FoundationModels

func callFoundationModel(prompt: String) throws -> String {
    let model = try SystemLanguageModel.default
    let session = try model.makeSession()
    let reply = try session.respond(to: prompt)
    return reply.text  // or whatever the API exposes
}
```

(Use the exact symbols/signatures from Apple’s latest docs; the above is just the structural idea.)

---

If you want, next step I can:

* Morph this into an **HTTP server in Swift** instead of a CLI.
* Or wrap this into an **Effect-TS service** in your Bun app for cleaner integration with the rest of your stack.

> if we're speakin JSON lets also spec out making a DVM out of this via NIP90 [links to NIP90]

Nice, let’s turn the bridge into a proper NIP-90 DVM spec.

I’ll do 3 layers:

1. **NIP-90 job kind** for “Foundation Models completion”
2. **Bridge JSON contract** (what Bun sends to the Swift binary & gets back)
3. **End-to-end example** (job request → Bun → Swift → job result)

You can plug this straight into `nostr-effect`.

---

## 1️⃣ Define a DVM job kind for Apple Foundation Models

Let’s pick a concrete kind:

* **Job request kind:** `5101` – `"foundation-models.completion"`
* **Job result kind:** `6101`

### 1.1. Job request event (`kind:5101`)

Semantic: “run a text completion / chat turn via Apple Foundation Models and return text or JSON”.

**Shape:**

```jsonc
{
  "kind": 5101,
  "content": "", // or encrypted payload if using encrypted params
  "tags": [
    // INPUT(S)
    ["i", "What is the capital of France?", "text", "", "prompt"],

    // Optional: system prompt (still text)
    ["i", "You are a helpful assistant.", "text", "", "system"],

    // OUTPUT FORMAT (we'll use JSON so bridge can be structured)
    ["output", "application/json"],

    // PARAMS (all optional)
    ["param", "model", "foundation.default"],
    ["param", "max_tokens", "512"],
    ["param", "temperature", "0.5"],
    ["param", "top_p", "0.9"],
    ["param", "stop", "\n\n"],

    // PAYMENT / ROUTING
    ["bid", "1000"], // msats the customer is willing to pay
    ["relays", "wss://relay1.example", "wss://relay2.example"],
    ["p", "<service-provider-pubkey>"],

    // Optional topic tags
    ["t", "foundation-models"],
    ["t", "llm"]
  ]
}
```

**Rules for your DVM:**

* You **MUST** understand at least:

  * `i` tag with `marker=prompt` (main user prompt)
  * `i` tag with `marker=system` (system prompt)
* You **SHOULD** support:

  * `param` tags: `model`, `max_tokens`, `temperature`, `top_p`, `stop`
* You **MAY** ignore unknown params.

Encrypted params follow NIP-90: customer encrypts `i` + `param` as NIP-04 into `content`, adds `["encrypted"]` tag + `["p", "<sp-pubkey>"]`. Your DVM decrypts before mapping to the bridge.

---

## 2️⃣ Bridge JSON contract (Bun ↔ Swift)

Replace the simple `{ prompt: string }` with a DVM-aware payload that’s still easy to evolve.

### 2.1. Request sent from Bun to Swift

```jsonc
{
  "type": "completion",
  "job_kind": 5101,
  "input": {
    "prompt": "What is the capital of France?",
    "system": "You are a helpful assistant."
  },
  "params": {
    "model": "foundation.default",
    "max_tokens": 512,
    "temperature": 0.5,
    "top_p": 0.9,
    "stop": ["\n\n"]
  },
  "nostr": {
    "request_event_id": "<job-request-id>",
    "customer_pubkey": "<customer-npub>",
    "bid_msat": 1000
  }
}
```

TypeScript types on the Bun side:

```ts
export type DvmBridgeRequest = {
  type: "completion";
  job_kind: 5101;
  input: {
    prompt: string;
    system?: string;
  };
  params?: {
    model?: string;
    max_tokens?: number;
    temperature?: number;
    top_p?: number;
    stop?: string[];
  };
  nostr?: {
    request_event_id: string;
    customer_pubkey: string;
    bid_msat?: number;
  };
};
```

### 2.2. Response returned from Swift to Bun

```jsonc
{
  "output": "The capital of France is Paris.",
  "usage": {
    "input_tokens": 23,
    "output_tokens": 9
  },
  "finish_reason": "stop",
  "model": "foundation.default"
}
```

TypeScript:

```ts
export type DvmBridgeResponse = {
  output?: string; // main textual output
  usage?: {
    input_tokens?: number;
    output_tokens?: number;
  };
  finish_reason?: "stop" | "length" | "content_filter" | string;
  model?: string;

  error?: {
    code: string;
    message: string;
  };
};
```

On **error**, Swift returns:

```jsonc
{
  "error": {
    "code": "bridge_failed",
    "message": "some description"
  }
}
```

---

## 3️⃣ Swift bridge updated for the DVM contract

Here’s an updated `main.swift` using the richer JSON:

```swift
import Foundation
// import FoundationModels // when you wire the real thing

struct NostrInfo: Codable {
    let request_event_id: String?
    let customer_pubkey: String?
    let bid_msat: Int?
}

struct InputPayload: Codable {
    let prompt: String
    let system: String?
}

struct ParamsPayload: Codable {
    let model: String?
    let max_tokens: Int?
    let temperature: Double?
    let top_p: Double?
    let stop: [String]?
}

struct BridgeRequest: Codable {
    let type: String
    let job_kind: Int
    let input: InputPayload
    let params: ParamsPayload?
    let nostr: NostrInfo?
}

struct UsagePayload: Codable {
    let input_tokens: Int?
    let output_tokens: Int?
}

struct ErrorPayload: Codable {
    let code: String
    let message: String
}

struct BridgeResponse: Codable {
    let output: String?
    let usage: UsagePayload?
    let finish_reason: String?
    let model: String?
    let error: ErrorPayload?
}

func readAllStdin() -> Data {
    var data = Data()
    while case let chunk = FileHandle.standardInput.availableData, !chunk.isEmpty {
        data.append(chunk)
    }
    return data
}

// MARK: - Foundation Models call

func callFoundationModel(request: BridgeRequest) throws -> BridgeResponse {
    let prompt = request.input.prompt
    let system = request.input.system

    // TODO: Replace with real FoundationModels invocation
    let combined = [
        system.map { "System: \($0)\n" } ?? "",
        "User: \(prompt)"
    ].joined()

    let outputText = "Stub FM response for:\n\(combined)"

    // When wired to the real model, you can fill usage + finish_reason
    return BridgeResponse(
        output: outputText,
        usage: UsagePayload(input_tokens: nil, output_tokens: nil),
        finish_reason: "stop",
        model: request.params?.model ?? "foundation.default",
        error: nil
    )
}

// MARK: - Main

do {
    let inputData = readAllStdin()
    let decoder = JSONDecoder()
    let req = try decoder.decode(BridgeRequest.self, from: inputData)

    guard req.type == "completion", req.job_kind == 5101 else {
        let err = BridgeResponse(
            output: nil,
            usage: nil,
            finish_reason: nil,
            model: nil,
            error: ErrorPayload(
                code: "unsupported_job",
                message: "Unsupported type \(req.type) / kind \(req.job_kind)"
            )
        )
        let data = try JSONEncoder().encode(err)
        FileHandle.standardOutput.write(data)
        FileHandle.standardOutput.write("\n".data(using: .utf8)!)
        exit(0)
    }

    let resp = try callFoundationModel(request: req)
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.prettyPrinted]

    let data = try encoder.encode(resp)
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write("\n".data(using: .utf8)!)
} catch {
    let err = BridgeResponse(
        output: nil,
        usage: nil,
        finish_reason: nil,
        model: nil,
        error: ErrorPayload(
            code: "bridge_failed",
            message: String(describing: error)
        )
    )
    let data = try! JSONEncoder().encode(err)
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write("\n".data(using: .utf8)!)
    exit(1)
}
```

---

## 4️⃣ Bun DVM handler: map NIP-90 ↔ bridge

This sits inside your `nostr-effect` service.

### 4.1. Parse the job request into `DvmBridgeRequest`

Sketch:

```ts
import type { Event } from "nostr-tools"; // or nostr-effect types
import { spawn } from "bun";

import type { DvmBridgeRequest, DvmBridgeResponse } from "./bridge-types";

function getTags(e: Event, name: string): string[][] {
  return e.tags.filter((t) => t[0] === name);
}

function firstTagValue(e: Event, name: string): string | undefined {
  return getTags(e, name)[0]?.[1];
}

export async function handleFoundationJob(event: Event): Promise<Event> {
  if (event.kind !== 5101) {
    throw new Error("Unsupported kind");
  }

  // Find prompt + system from "i" tags
  const iTags = getTags(event, "i");

  const prompt =
    iTags.find((t) => t[4] === "prompt")?.[1] ??
    iTags[0]?.[1] ??
    "";

  const system = iTags.find((t) => t[4] === "system")?.[1];

  // Params
  const paramTags = getTags(event, "param");
  const params: Record<string, string> = {};
  for (const tag of paramTags) {
    const key = tag[1];
    const value = tag[2];
    params[key] = value;
  }

  const bidMsat = Number(firstTagValue(event, "bid") ?? "0") || undefined;

  const bridgeReq: DvmBridgeRequest = {
    type: "completion",
    job_kind: 5101,
    input: { prompt, system },
    params: {
      model: params["model"],
      max_tokens: params["max_tokens"]
        ? Number(params["max_tokens"])
        : undefined,
      temperature: params["temperature"]
        ? Number(params["temperature"])
        : undefined,
      top_p: params["top_p"] ? Number(params["top_p"]) : undefined,
      stop: params["stop"] ? [params["stop"]] : undefined
    },
    nostr: {
      request_event_id: event.id,
      customer_pubkey: event.pubkey,
      bid_msat: bidMsat
    }
  };

  const bridgeResp = await callSwiftBridge(bridgeReq);

  return makeResultEvent(event, bridgeResp);
}

async function callSwiftBridge(req: DvmBridgeRequest): Promise<DvmBridgeResponse> {
  const proc = Bun.spawn(["./foundation-bridge"], {
    stdin: "pipe",
    stdout: "pipe",
    stderr: "pipe"
  });

  const encoder = new TextEncoder();
  const stdinJson = JSON.stringify(req) + "\n";
  proc.stdin!.write(encoder.encode(stdinJson));
  proc.stdin!.end();

  const stdoutData = await new Response(proc.stdout).text();
  const stderrData = await new Response(proc.stderr).text();
  if (stderrData.trim()) console.error("bridge stderr:", stderrData);

  const parsed: DvmBridgeResponse = JSON.parse(stdoutData);

  if (parsed.error) {
    throw new Error(`${parsed.error.code}: ${parsed.error.message}`);
  }

  if (!parsed.output) {
    throw new Error("bridge did not return output");
  }

  return parsed;
}
```

### 4.2. Turn `DvmBridgeResponse` into a `kind:6101` job result

```ts
import { getEventHash, signEvent } from "nostr-tools";

function makeResultEvent(
  request: Event,
  resp: DvmBridgeResponse
): Event {
  const contentPayload = {
    output: resp.output,
    usage: resp.usage,
    finish_reason: resp.finish_reason,
    model: resp.model
  };

  const content = JSON.stringify(contentPayload);

  const tags: string[][] = [
    ["request", JSON.stringify(request)], // full request event
    ["e", request.id, ""],
    ["p", request.pubkey],
    ["i", resp.output ?? ""], // optional: echo input/output
    ["amount", String(request.tags.find((t) => t[0] === "bid")?.[1] ?? "0")]
  ];

  const result: Event = {
    kind: 6101,
    pubkey: "<your-service-provider-pubkey>",
    content,
    tags,
    created_at: Math.floor(Date.now() / 1000),
    id: "" as any,
    sig: "" as any
  };

  result.id = getEventHash(result);
  result.sig = signEvent(result, "<your-service-provider-privkey>");

  return result;
}
```

If you want to support **encrypted output**, do:

* Encrypt `contentPayload` via NIP-04 using `request.pubkey`.
* Put ciphertext into `content`.
* Add `["encrypted"]` tag.
* Omit `["i", ...]` or avoid including cleartext.

---

## 5️⃣ NIP-89 announcement for this DVM

So clients can discover you:

```jsonc
{
  "kind": 31990,
  "pubkey": "<your-service-provider-pubkey>",
  "content": "{
    \"name\": \"Apple Foundation Models DVM\",
    \"about\": \"Runs Apple on-device Foundation Models (macOS 26/M-series) via NIP-90.\"
  }",
  "tags": [
    ["k", "5101"],
    ["t", "foundation-models"],
    ["t", "llm"]
  ]
}
```

---

If you want, next step we can tighten this into a tiny `nostr-effect` service module (`FoundationModelsDvmService`) with Effect-TS types so you can drop it straight into your Bun worker.
