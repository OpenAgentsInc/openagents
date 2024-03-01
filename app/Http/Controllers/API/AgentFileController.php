<?php

namespace App\Http\Controllers\API;

use App\Http\Controllers\Controller;
use App\Models\Agent;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;

/**
 * @OA\Tag(
 *     name="AgentFile",
 *     description="Operations about agent files"
 * )
 */
class AgentFileController extends Controller
{
    /**
     * Display a listing of the resource.
     */
    public function index()
    {
        //
    }

    /**
     * @OA\Post(
     *     path="/agents/{agentId}/files",
     *     tags={"AgentFile"},
     *     summary="Create agent file",
     *     operationId="storeAgentFile",
     *
     *     @OA\Parameter(
     *         name="agentId",
     *         in="path",
     *         description="ID of the agent to add file for",
     *         required=true,
     *
     *         @OA\Schema(
     *             type="integer"
     *         )
     *     ),
     *
     *     @OA\RequestBody(
     *         required=true,
     *         description="Upload new file for agent",
     *
     *         @OA\MediaType(
     *             mediaType="multipart/form-data",
     *
     *             @OA\Schema(
     *
     *                 @OA\Property(
     *                     property="file",
     *                     description="File to upload",
     *                     type="string",
     *                     format="binary"
     *                 ),
     *                 @OA\Property(
     *                     property="description",
     *                     type="string"
     *                 )
     *             )
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=200,
     *         description="File added successfully",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(
     *                 property="success",
     *                 type="boolean"
     *             ),
     *             @OA\Property(
     *                 property="message",
     *                 type="string"
     *             ),
     *             @OA\Property(
     *                 property="data",
     *                 type="object",
     *                 @OA\Property(
     *                     property="file_id",
     *                     type="integer"
     *                 ),
     *                 @OA\Property(
     *                     property="agent_id",
     *                     type="integer"
     *                 )
     *             )
     *         )
     *     ),
     *
     *     security={{"bearerAuth":{}}}
     * )
     */
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
            'user_id' => Auth::id(), // Assuming the file is associated with the authenticated user
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
