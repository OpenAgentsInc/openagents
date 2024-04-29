<?php

namespace App\Services;

class AgentChatService
{
    public function chatWithAgent($thread, $agent, $message)
    {
        $user = auth()->user();

        // If the user has no agent credit, we can't chat with them
        if ($user->credits <= 0) {
            return false;
        }

        // Deduct credit from the user
        $user->credits -= 1;
        $user->save();

        // Do the actual chat
        return true;
    }
}
