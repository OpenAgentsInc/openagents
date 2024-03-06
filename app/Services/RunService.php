<?php

namespace App\Services;

use App\Models\Run;

class RunService
{
    /**
     * Create a run for the given agent/flow/thread with the given input and streaming function
     */
    public function triggerRun($data)
    {
        // Extract the data
        $agent = $data['agent'];
        $flow = $data['flow'];
        $thread = $data['thread'];
        $input = $data['input'];
        $streamingFunction = $data['streamingFunction'];

        // Create a new run
        $run = Run::create([
            'agent_id' => $agent->id,
            'flow_id' => $flow->id,
            'thread_id' => $thread->id,
            'input' => $input,
        ]);

        // Trigger the run and return its result as output
        return $run->trigger($streamingFunction);
    }
}
