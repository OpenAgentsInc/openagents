<?php

namespace App\Services;

class ThreadService
{
    public function createThread(int $agentId): array
    {
        // Create a new thread and return its details
        return [
            'id' => 1,
            'agent_id' => $agentId,
        ];
    }
}
