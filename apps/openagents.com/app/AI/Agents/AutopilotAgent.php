<?php

namespace App\AI\Agents;

use Illuminate\Support\Stringable;
use Laravel\Ai\Concerns\RemembersConversations;
use Laravel\Ai\Contracts\Agent;
use Laravel\Ai\Contracts\Conversational;
use Laravel\Ai\Promptable;

class AutopilotAgent implements Agent, Conversational
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
PROMPT;
    }
}
