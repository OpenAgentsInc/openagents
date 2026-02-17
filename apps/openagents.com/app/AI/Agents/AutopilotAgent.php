<?php

namespace App\AI\Agents;

use App\AI\Tools\AutopilotToolResolver;
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
- For paid API requests, use `lightning_l402_fetch` with a strict `maxSpendMsats` (or temporary alias `maxSpendSats`) and appropriate `scope`.
- Keep `requireApproval=true` (or temporary alias `approvalRequired`) so spending is gated.
- If `lightning_l402_fetch` returns `status=approval_requested`, ask user to approve.
- Only after explicit user approval, call `lightning_l402_approve` with the returned `taskId`.
- After approval completes, summarize the paid result and include payment proof reference when available.
PROMPT;
    }

    public function tools(): iterable
    {
        return resolve(AutopilotToolResolver::class)->forCurrentAutopilot();
    }

    /**
     * Always use Vercel AI Gateway for chat (never OpenRouter).
     */
    public function provider(): string
    {
        return 'ai_gateway';
    }

    /**
     * Default model for text generation (Vercel AI Gateway via ai_gateway).
     */
    public function model(): string
    {
        return self::defaultModel();
    }

    /**
     * Primary model (used first).
     */
    public static function defaultModel(): string
    {
        return 'google/gemini-3-flash';
    }

    /**
     * Backup model if primary fails (e.g. rate limit or model down).
     */
    public static function backupModel(): string
    {
        return 'xai/grok-4.1-fast-non-reasoning';
    }
}
