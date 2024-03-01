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
     * Store a newly created resource in storage.
     */
    public function store(Request $request)
    {
        //
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
