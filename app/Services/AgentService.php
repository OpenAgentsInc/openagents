<?php

namespace App\Services;

use App\Models\Agent;
use Exception;
use Illuminate\Database\Eloquent\Collection;
use Illuminate\Support\Facades\Auth;

class AgentService
{
    /**
     * Retrieves all agents owned by the currently authenticated user.
     *
     * This method can be expanded to include pagination or filtering
     * based on application requirements.
     *
     * @return Collection|static[]
     */
    public function getAllAgentsByUser(): Collection|static
    {
        $userId = Auth::id(); // Get the currently authenticated user's ID

        return Agent::where('user_id', $userId)->get();
    }

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

    /**
     * Finds an agent by its ID.
     *
     * @param  int|string  $id  The ID of the agent to find.
     * @return Agent|null The found agent or null if not found.
     */
    public function findAgentById($id): ?Agent
    {
        return Agent::find($id);
    }

    /**
     * Updates an agent with the given details.
     *
     * @param  int|string  $id  The ID of the agent to update.
     * @param  array  $data  The data to update the agent with.
     * @return Agent|null The updated agent object or null if the update failed.
     */
    public function updateAgent($id, array $data): ?Agent
    {
        $agent = Agent::find($id);
        if (! $agent) {
            return null;
        }

        $agent->update($data);

        return $agent;
    }

    /**
     * Deletes an agent by its ID.
     *
     * @param  int|string  $id  The ID of the agent to delete.
     * @return bool True if the agent was deleted successfully, false otherwise.
     */
    public function deleteAgent($id): bool
    {
        $agent = Agent::find($id);
        if (! $agent) {
            return false;
        }

        return $agent->delete();
    }
}
