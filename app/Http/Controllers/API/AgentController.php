<?php

namespace App\Http\Controllers\API;

use App\Http\Controllers\Controller;
use App\Services\AgentService;
use Exception;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Validator;

class AgentController extends Controller
{
    protected $agentService;

    // Inject AgentService into the controller
    public function __construct(AgentService $agentService)
    {
        $this->agentService = $agentService;
    }

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
        // Validate request input
        $validator = Validator::make($request->all(), [
            'name' => 'required|string',
            'description' => 'required|string',
            'instructions' => 'required|string',
            'welcome_message' => 'sometimes|string',
        ]);

        if ($validator->fails()) {
            return response()->json(['success' => false, 'message' => 'Validation errors', 'errors' => $validator->errors()], 400);
        }

        try {
            // Use the agent service to create a new agent
            $agent = $this->agentService->createAgent(
                $request->name,
                $request->description,
                $request->instructions,
                $request->welcome_message // This can be null, the service handles it
            );

            // Return the response structured as expected by the test
            return response()->json([
                'success' => true,
                'message' => 'Agent created successfully.',
                'data' => [
                    'agent_id' => $agent->id,
                ],
            ], 201);
        } catch (Exception $e) {
            // Handle any exceptions, such as authentication failures
            return response()->json(['success' => false, 'message' => $e->getMessage()], 403);
        }
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
