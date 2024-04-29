<?php

namespace App\Services;

class AgentChatService
{
    public function chatWithAgent($thread, $agent, $message)
    {
        // If the agent has no credit, we can't chat with them
        if ($agent->credit <= 0) {
            return false;
        }

        // Deduct credit from the agent
        $agent->credit -= 1;
        $agent->save();

        // Do the actual chat
        return true;
    }
}
