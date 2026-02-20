# Autopilot Bootstrap Flow and Greeting Fix

This doc explains how the **bootstrap “ask user handle”** stage works in the web app and worker, how we avoid generic assistant greetings (e.g. “Hello! How can I assist you today?”), and how we keep asking for a name until the user provides one. It also describes the Khala + DSE flow that persists the handle and advances the stage.

See [bootstrap-plan.md](../bootstrap-plan.md) for the overall bootstrap design (state machine, Blueprint records, schema).

---

## Problem (What Was Wrong)

1. **Web app (Khala-first stream)**  
   The stream in `apps/web/src/effuse-host/autopilot.ts` used a **fixed** system prompt and did **not** load bootstrap state. The model never knew it was in “ask for name,” so it fell back to generic greetings like “Hello! How can I assist you today?” when the user said “hi.”

2. **No “stay on question” rule**  
   There was no instruction to keep asking “What shall I call you?” until the user gave a name, or to avoid generic greetings during bootstrap.

3. **SelectTool result unused**  
   The DSE SelectTool ran for telemetry but its result did not change behavior; the main model call used `toolChoice: "none"` and no tools, so the model had no bootstrap context.

---

## High-Level Behavior (After Fix)

- **During `ask_user_handle`**  
  - The system prompt explicitly tells the model: do **not** say “Hello! How can I assist you today?” or similar; if the user has not given a name (e.g. “hi”, “hello”), respond **only** by re-asking “What shall I call you?” and keep asking until they give a name.

- **When the user gives a name**  
  - The web app runs the **ExtractUserHandle** DSE on the last user message. If it returns a handle that is not `"Unknown"`, the app calls the Khala mutation **applyBootstrapUserHandle**, which updates the Blueprint user doc and sets `bootstrapState.stage` to `ask_agent_name`. The next turn then proceeds to the next bootstrap question.

---

## Where This Is Implemented

| Concern | Web app (Khala-first) | Autopilot worker (DO) |
|--------|------------------------|------------------------|
| **System prompt** | `apps/web/src/effuse-host/autopilot.ts`: `concatTextFromPromptMessages(..., blueprint)` + `BOOTSTRAP_ASK_USER_HANDLE_SYSTEM` when `stage === "ask_user_handle"` | `apps/autopilot-worker/src/server.ts`: `SYSTEM_PROMPT_BASE` includes “Never say generic greetings…” and “During bootstrap, stay on the current question…” |
| **Blueprint load** | Fetched at stream start via `api.autopilot.blueprint.getBlueprint` and passed into prompt builder | Loaded from DO SQLite; used for `buildSystemPrompt` and forced tool choice |
| **Re-ask / no greeting** | Injected bootstrap block in system prompt when `status !== "complete"` and `stage === "ask_user_handle"` | Same rule in base system prompt |
| **Persist handle + advance** | ExtractUserHandle DSE → `api.autopilot.blueprint.applyBootstrapUserHandle` | Tool `bootstrap_set_user_handle` (forced when stage is `ask_user_handle`) |

---

## Web App Flow (Khala-First Stream)

1. **Stream start**  
   - Load message snapshot and **blueprint** from Khala (`getThreadSnapshot`, `getBlueprint`).

2. **Prompt build**  
   - `concatTextFromPromptMessages(tail, blueprint)` builds the system prompt.  
   - If `blueprint.bootstrapState.status !== "complete"` and `blueprint.bootstrapState.stage === "ask_user_handle"`, it appends **bootstrap instructions**:  
     - Do not say generic greetings like “Hello! How can I assist you today?”  
     - If the user did not give a name, respond only by re-asking “What shall I call you?”  
     - Keep asking until they give a name, then confirm and move on.

3. **DSE block (best-effort, does not block reply)**  
   - Runs SelectTool (for telemetry).  
   - If `stage === "ask_user_handle"`, runs **ExtractUserHandle** on the last user message.  
   - If the extracted handle is non-empty and not `"Unknown"`, calls Khala mutation **applyBootstrapUserHandle(threadId, anonKey?, handle)**.  
   - That mutation updates `docs.user.addressAs` and `docs.user.name`, and sets `bootstrapState.stage` to `"ask_agent_name"`, so the next turn uses the next bootstrap stage.

4. **Model call**  
   - `AiLanguageModel.streamText` with the bootstrap-aware prompt and `toolChoice: "none"` (web app still has no tools in the stream). The model’s reply is therefore guided only by the system prompt (re-ask for name, no generic greeting).

---

## Autopilot Worker (DO) Flow

When the execution plane is the Durable Object (e.g. future or alternate path):

- **Bootstrap forces a tool**  
  For `ask_user_handle`, the worker sets `toolChoice: { tool: "bootstrap_set_user_handle" }`, so the model must call that tool with the user’s handle.

- **System prompt**  
  `SYSTEM_PROMPT_BASE` includes:  
  “Never say generic greetings like ‘Hello! How can I assist you today?’ or ‘How can I help?’. During bootstrap, stay on the current question until the user answers it.”

So in the DO path the model is both instructed and constrained by tool choice; in the web app path it is instructed only by the prompt (no tools).

---

## Khala: applyBootstrapUserHandle

- **Location:** `apps/web/khala/autopilot/blueprint.ts`  
- **Behavior:**  
  - Asserts thread access.  
  - Loads the current Blueprint for the thread.  
  - If `bootstrapState.status === "complete"` or `bootstrapState.stage !== "ask_user_handle"`, returns `{ ok: true, applied: false }` (idempotent).  
  - Otherwise: patches `docs.user` with `addressAs` and `name`, sets `bootstrapState.stage` to `"ask_agent_name"`, and saves the Blueprint.  
- **Called from:** Web app stream, after ExtractUserHandle returns a valid handle.

---

## DSE Signatures Involved

- **SelectTool** (`@openagents/autopilot/blueprint/SelectTool.v1`)  
  - Used in the web app for routing/telemetry only; its `action: "none"` (e.g. for “hi”) does **not** change tool choice or prompt in the Khala-first flow.

- **ExtractUserHandle** (`@openagents/autopilot/bootstrap/ExtractUserHandle.v1`)  
  - Input: `{ message: string }`.  
  - Output: `{ handle: string }` (e.g. “Chris” from “Call me Chris” or “Chris”).  
  - Used in the web app when `stage === "ask_user_handle"` to decide whether to call **applyBootstrapUserHandle**.

---

## Testing

- **Chat-streaming Khala test** (`apps/web/tests/worker/chat-streaming-khala.test.ts`):  
  - Khala mock handles **getBlueprint** (returns `{ ok: true, blueprint: null, updatedAtMs: 0 }`).  
  - Khala mock handles **applyBootstrapUserHandle** (returns `{ ok: true, applied: true, updatedAtMs }`).  
  So the stream can run without real Khala and the new queries/mutations are covered.

---

## Summary

| Issue | Fix |
|-------|-----|
| Model said “Hello! How can I assist you today?” | Bootstrap-aware system prompt (web + worker): never use generic greetings; during bootstrap stay on the current question. |
| Model did not re-ask for a name after “hi” | Explicit instruction: if the user has not given a name, respond only by re-asking “What shall I call you?” until they do. |
| Handle not persisted in web app | ExtractUserHandle DSE + **applyBootstrapUserHandle** Khala mutation when stage is `ask_user_handle` and extracted handle is valid. |
| Next turn still in ask_user_handle | Mutation sets `bootstrapState.stage` to `"ask_agent_name"` so the next turn gets the next bootstrap stage. |
