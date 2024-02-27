<?php

namespace App\Services;

use App\Models\Agent;
use Exception;
use Illuminate\Support\Facades\Auth;

class AgentService
{
    /**
     * Creates an agent with the given details.
     *
     * @param  string  $name  The name of the agent.
     * @param  string  $description  A brief description of the agent.
     * @param  string  $instructions  Detailed instructions on how the agent operates.
     * @param  string|null  $welcomeMessage  An optional welcome message for the agent.
     * @return Agent The created Agent object.
     *
     * @throws Exception
     */
    public function createAgent(string $name, string $description, string $instructions, ?string $welcomeMessage = null): Agent
    {
        // Assuming 'user_id' is required to associate an agent with a user.
        // Ensure the user is authenticated before creating an agent.
        if (! Auth::check()) {
            throw new Exception('User must be authenticated to create an agent.');
        }

        // Create and return the new agent.
        return Agent::create([
            'name' => $name,
            'description' => $description,
            'instructions' => $instructions,
            'welcome_message' => $welcomeMessage,
            'user_id' => Auth::id(), // Or another way to obtain the current user's ID, depending on your auth system
        ]);
    }
}
