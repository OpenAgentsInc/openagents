<?php

namespace App\Services;

use App\Models\Thread;

class ThreadService
{
    public function createThread(int $agentId): array
    {
        // Create new Thread and attach this Agent
        $thread = Thread::create();
        $thread->agents()->attach($agentId);

        return [
            'id' => 1,
            'agent_id' => $agentId,
        ];
    }
}
