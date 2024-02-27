<?php

namespace App\Http\Controllers\API;

use App\Http\Controllers\Controller;
use App\Models\Agent;
use Illuminate\Http\Request;

class AgentFileController extends Controller
{
    public function store(Request $request, Agent $agent)
    {
        $request->validate([
            'file' => 'required|file',
            'description' => 'required|string',
        ]);

        // Handle file upload
        $file = $request->file('file');
        $filePath = $file->store('agent_files'); // Adjust the path as needed

        // Create the file entry in your database here
        // Assuming you have a File model that is related to the Agent model
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
