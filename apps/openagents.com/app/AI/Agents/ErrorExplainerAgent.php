<?php

namespace App\AI\Agents;

use Illuminate\Support\Stringable;
use Laravel\Ai\Contracts\Agent;
use Laravel\Ai\Promptable;

/**
 * Minimal agent used to explain run errors to the user.
 * No tools, no conversation — single turn only.
 */
class ErrorExplainerAgent implements Agent
{
    use Promptable;

    public function instructions(): Stringable|string
    {
        return <<<'PROMPT'
You are Autopilot (an agent product of OpenAgents). The system encountered an error while generating a response.

Your task: explain to the user what went wrong in simple, friendly terms. Be brief. Do not expose internal implementation details unless they help the user (e.g. "the model's tool call failed" is fine; stack traces or raw API names are not). If the user can retry or do something differently, say so briefly.
PROMPT;
    }

    public function provider(): string
    {
        return 'ai_gateway';
    }

    public function model(): string
    {
        return AutopilotAgent::defaultModel();
    }
}
