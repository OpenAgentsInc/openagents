<?php

namespace App\Services;

use App\Models\Run;

class RunService
{
    /**
     * Create a run for the given agent/flow/thread ID with the given input and streaming function
     */
    public function createRun(int $agentId, int $flowId, int $threadId, string $input, callable $streamingFunction): void
    {
        // Create a new run
        $run = Run::create([
            'agent_id' => $agentId,
            'flow_id' => $flowId,
            'thread_id' => $threadId,
            'input' => $input,
        ]);
        $run->trigger();

        // Run the streaming function
        $streamingFunction($run);
    }
}
