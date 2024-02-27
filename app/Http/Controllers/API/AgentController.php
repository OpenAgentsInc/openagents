<?php

namespace App\Http\Controllers\API;

use App\Http\Controllers\Controller;
use App\Models\Agent;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Http\Response;

class AgentController extends Controller
{
    /**
     * Display a listing of the resource.
     */
    public function index()
    {
        //
    }

    /**
     * Store a newly created agent in storage.
     *
     * This method handles the creation of a new agent based on the provided
     * name, description, and instructions. It validates the request data
     * and returns a JSON response indicating the success or failure of
     * the agent creation process.
     *
     * @return JsonResponse
     */
    public function store(Request $request)
    {
        // Validate the request input
        $validatedData = $request->validate([
            'name' => 'required|string',
            'description' => 'required|string',
            'instructions' => 'required|string',
        ]);

        // Create a new agent with the validated data
        $agent = Agent::create([
            'name' => $validatedData['name'],
            'description' => $validatedData['description'],
            'instructions' => $validatedData['instructions'],
            // Assuming 'user_id' is required for ownership linking
            'user_id' => $request->user()->id,
        ]);

        // Return a JSON response with the created agent and a 201 status code for successful creation
        return response()->json([
            'success' => true,
            'message' => 'Agent created successfully.',
            'data' => [
                'agent_id' => $agent->id,
            ],
        ], Response::HTTP_CREATED); // 201 status code
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
