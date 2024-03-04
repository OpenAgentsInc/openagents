<?php

/**
 * AgentThreadController
 *
 * Controller for the AgentThread model
 *
 * Agents may belong to many Threads.
 * Threads can have many Agents.
 */

namespace App\Http\Controllers\API;

use App\Http\Controllers\Controller;
use App\Models\Agent;
use App\Models\Thread;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class AgentThreadController extends Controller
{
    /**
     * Display a listing of the resource.
     */
    public function index($agentId)
    {
        // Find all threads that the agent is a part of (via many-to-many relationship)
        $agent = Agent::find($agentId);
        $threads = $agent->threads;

        return response()->json([
            'success' => true,
            'message' => 'Agent threads retrieved successfully',
            'data' => $threads->toArray(),
        ]);
    }

    /**
     * Store a newly created association between agent and thread in storage.
     *
     * @return JsonResponse
     */
    public function store(Request $request, int $agentId)
    {
        // Validate the incoming request data
        $validatedData = $request->validate([
            'thread_id' => 'required|exists:threads,id', // Ensure 'thread_id' field is provided and exists in the 'threads' table
        ]);

        // Retrieve the agent by id, or fail with 404 if not found
        $agent = Agent::findOrFail($agentId);

        // Find the thread by ID
        $thread = Thread::findOrFail($validatedData['thread_id']);

        // Associate the thread with the agent
        // This assumes that the Agent model has a threads() relationship method defined
        $agent->threads()->attach($thread->id);

        // Return a JSON response confirming the association
        return response()->json([
            'success' => true,
            'message' => 'Agent associated with thread successfully',
            'data' => [
                'agent_id' => $agent->id,
                'thread_id' => $thread->id,
            ],
        ], 200); // HTTP status code 200 indicates success
    }

    /**
     * Display the specified resource.
     */
    public function show(string $id)
    {
        //
    }

    /**
     * Update the specified resource in storage.
     */
    public function update(Request $request, string $id)
    {
        //
    }

    /**
     * Remove the specified resource from storage.
     */
    public function destroy(string $id)
    {
        //
    }
}
