<?php

namespace App\AI\Agents;

use App\AI\Tools\ToolRegistry;
use Illuminate\Support\Stringable;
use Laravel\Ai\Concerns\RemembersConversations;
use Laravel\Ai\Contracts\Agent;
use Laravel\Ai\Contracts\Conversational;
use Laravel\Ai\Contracts\HasTools;
use Laravel\Ai\Promptable;

class AutopilotAgent implements Agent, Conversational, HasTools
{
    use Promptable;
    use RemembersConversations;

    public function instructions(): Stringable|string
    {
        return <<<'PROMPT'
You are Autopilot.

Style:
- Concise
- Direct
- Pragmatic

Rules:
- If the user asks for code, prefer concrete steps and copy/paste-ready snippets.
- If you are unsure, ask a focused clarifying question.

Tooling:
- You can call tools when it will materially improve correctness or speed.
- Prefer tools for deterministic operations (time, formatting, simple transforms).

Lightning / L402 workflow:
- For paid API requests, use `lightning_l402_fetch` with a strict `maxSpendSats` and appropriate `scope`.
- Keep `approvalRequired=true` (default) so spending is gated.
- If `lightning_l402_fetch` returns `status=approval_requested`, ask user to approve.
- Only after explicit user approval, call `lightning_l402_approve` with the returned `taskId`.
- After approval completes, summarize the paid result and include payment proof reference when available.
PROMPT;
    }

    public function tools(): iterable
    {
        return resolve(ToolRegistry::class)->all();
    }
}
