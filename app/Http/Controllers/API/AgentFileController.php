<?php

namespace App\Http\Controllers\API;

use App\Http\Controllers\Controller;
use App\Models\Agent;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;

class AgentFileController extends Controller
{
    public function store(Request $request, $agentId)
    {
        // First, find the agent by ID
        $agent = Agent::findOrFail($agentId);

        // Check if the authenticated user is the owner of the agent
        if ($agent->user_id !== Auth::id()) {
            return response()->json(['message' => 'You are not authorized to add files to this agent.'], 403);
        }

        // Validate the incoming request
        $request->validate([
            'file' => 'required|file',
            'description' => 'required|string',
        ]);

        // Handle file upload
        $file = $request->file('file');
        $filePath = $file->store('agent_files'); // Adjust the path as needed

        // Assuming you have a File model associated with the Agent model
        // and the agents table has a relationship set up with files
        $uploadedFile = $agent->files()->create([
            'path' => $filePath,
            'description' => $request->description,
            // Add other necessary fields here
        ]);

        return response()->json([
            'success' => true,
            'message' => 'File added to agent successfully.',
            'data' => [
                'file_id' => $uploadedFile->id,
                'agent_id' => $agent->id,
            ],
        ]);
    }
}
