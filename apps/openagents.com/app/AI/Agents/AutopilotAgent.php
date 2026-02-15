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
PROMPT;
    }

    public function tools(): iterable
    {
        return resolve(ToolRegistry::class)->all();
    }
}
