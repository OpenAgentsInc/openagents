<?php

namespace App\Http\Controllers\API;

use App\Http\Controllers\Controller;
use App\Services\AgentService;
use Exception;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Validator;

class AgentController extends Controller
{
    protected AgentService $agentService;

    public function __construct(AgentService $agentService)
    {
        $this->agentService = $agentService;
    }

    /**
     * @OA\Get(
     *     path="/agents",
     *     tags={"Agent"},
     *     summary="List agents",
     *     description="Returns a list of agents owned by the user.",
     *     operationId="listAgents",
     *
     *     @OA\Response(
     *         response=200,
     *         description="Successful operation",
     *
     *         @OA\JsonContent(
     *             type="object",
     *
     *             @OA\Property(
     *                 property="data",
     *                 type="array",
     *
     *                 @OA\Items(ref="#/components/schemas/Agent")
     *             ),
     *
     *             @OA\Property(property="success", type="boolean"),
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=500,
     *         description="Internal Server Error",
     *
     *         @OA\JsonContent(
     *             type="object",
     *
     *             @OA\Property(property="success", type="boolean"),
     *             @OA\Property(property="message", type="string"),
     *         )
     *     ),
     *
     *     security={{"bearerAuth":{}}}
     * )
     */
    public function index()
    {
        try {
            // Fetch all agents using the agent service
            $agents = $this->agentService->getAllAgentsByUser();

            // Return the list of agents
            return response()->json(['success' => true, 'data' => $agents], 200);
        } catch (Exception $e) {
            // Handle any exceptions by returning a 500 Internal Server Error
            return response()->json(['success' => false, 'message' => 'An unexpected error occurred'], 500);
        }
    }

    /**
     * @OA\Post(
     *     path="/agents",
     *     tags={"Agent"},
     *     summary="Create agent",
     *     operationId="createAgent",
     *
     *     @OA\RequestBody(
     *         required=true,
     *         description="Agent information",
     *
     *         @OA\JsonContent(
     *             required={"name","description","instructions"},
     *
     *             @OA\Property(property="name", type="string"),
     *             @OA\Property(property="description", type="string"),
     *             @OA\Property(property="instructions", type="string"),
     *             @OA\Property(property="welcome_message", type="string")
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=200,
     *         description="Agent created",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(property="success", type="boolean"),
     *             @OA\Property(property="message", type="string"),
     *             @OA\Property(
     *                 property="data",
     *                 type="object",
     *                 @OA\Property(property="agent_id", type="integer")
     *             )
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=400,
     *         description="Bad Request",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(property="success", type="boolean"),
     *             @OA\Property(property="message", type="string"),
     *             @OA\Property(property="errors", type="object")
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=500,
     *         description="Internal Server Error",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(property="success", type="boolean"),
     *             @OA\Property(property="message", type="string"),
     *         )
     *     ),
     *
     *     security={{"bearerAuth":{}}}
     * )
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
            return response()->json(['success' => false, 'message' => 'Validation errors', 'errors' => $validator->errors()], 422);
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
            return response()->json(['success' => true, 'message' => 'Agent created successfully', 'data' => ['agent_id' => $agent->id]]);
        } catch (Exception $e) {
            // Handle any exceptions, such as authentication failures
            return response()->json(['success' => false, 'message' => $e->getMessage()], 500);
        }
    }

    /**
     * @OA\Get(
     *     path="/agents/{id}",
     *     tags={"Agent"},
     *     summary="Retrieve agent",
     *     description="Retrieves an agent.",
     *     operationId="getAgentById",
     *
     *     @OA\Parameter(
     *         name="id",
     *         in="path",
     *         description="ID of agent to return",
     *         required=true,
     *
     *         @OA\Schema(
     *             type="integer"
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=200,
     *         description="Successful operation",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(
     *                 property="data",
     *                 ref="#/components/schemas/Agent"
     *             ),
     *             @OA\Property(property="success", type="boolean")
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=404,
     *         description="Agent not found",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(property="success", type="boolean"),
     *             @OA\Property(property="message", type="string"),
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=500,
     *         description="Internal Server Error",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(property="success", type="boolean"),
     *             @OA\Property(property="message", type="string"),
     *         )
     *     ),
     *
     *     security={{"bearerAuth":{}}}
     * )
     */
    public function show($id)
    {
        try {
            $agent = $this->agentService->findAgentById($id);

            if (! $agent) {
                return response()->json(['success' => false, 'message' => 'Agent not found'], 404);
            }

            return response()->json(['success' => true, 'data' => $agent]);
        } catch (Exception $e) {
            return response()->json(['success' => false, 'message' => $e->getMessage()], 500);
        }
    }

    /**
     * @OA\Put(
     *     path="/agents/{id}",
     *     tags={"Agent"},
     *     summary="Modify agent",
     *     operationId="updateAgent",
     *
     *     @OA\Parameter(
     *         name="id",
     *         in="path",
     *         description="ID of agent that needs to be updated",
     *         required=true,
     *
     *         @OA\Schema(
     *             type="integer"
     *         )
     *     ),
     *
     *     @OA\RequestBody(
     *         required=true,
     *         description="Agent data to update",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(
     *                 property="name",
     *                 type="string"
     *             ),
     *             @OA\Property(
     *                 property="description",
     *                 type="string"
     *             ),
     *             @OA\Property(
     *                 property="instructions",
     *                 type="string"
     *             ),
     *             @OA\Property(
     *                 property="welcome_message",
     *                 type="string"
     *             )
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=200,
     *         description="Agent updated",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(
     *                 property="data",
     *                 type="object",
     *                 ref="#/components/schemas/Agent"
     *             ),
     *             @OA\Property(property="success", type="boolean"),
     *             @OA\Property(property="message", type="string")
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=400,
     *         description="Bad Request",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(property="success", type="boolean"),
     *             @OA\Property(property="message", type="string"),
     *             @OA\Property(property="errors", type="object")
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=404,
     *         description="Agent not found",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(property="success", type="boolean"),
     *             @OA\Property(property="message", type="string"),
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=500,
     *         description="Internal Server Error",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(property="success", type="boolean"),
     *             @OA\Property(property="message", type="string"),
     *         )
     *     ),
     *
     *     security={{"bearerAuth":{}}}
     * )
     */
    public function update(Request $request, $id)
    {
        // Similar validation as in the store method
        $validator = Validator::make($request->all(), [
            'name' => 'sometimes|string',
            'description' => 'sometimes|string',
            'instructions' => 'sometimes|string',
            'welcome_message' => 'sometimes|string',
        ]);

        if ($validator->fails()) {
            return response()->json(['success' => false, 'message' => 'Validation errors', 'errors' => $validator->errors()], 400);
        }

        try {
            $agent = $this->agentService->updateAgent($id, $request->all());

            if (! $agent) {
                return response()->json(['success' => false, 'message' => 'Agent not found'], 404);
            }

            return response()->json(['success' => true, 'message' => 'Agent updated successfully.', 'data' => $agent], 200);
        } catch (Exception $e) {
            return response()->json(['success' => false, 'message' => $e->getMessage()], 500);
        }
    }

    /**
     * @OA\Delete(
     *     path="/agents/{id}",
     *     tags={"Agent"},
     *     summary="Delete agent",
     *     operationId="deleteAgent",
     *
     *     @OA\Parameter(
     *         name="id",
     *         in="path",
     *         description="Agent id to delete",
     *         required=true,
     *
     *         @OA\Schema(
     *             type="integer"
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=200,
     *         description="Agent deleted",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(property="success", type="boolean"),
     *             @OA\Property(property="message", type="string")
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=404,
     *         description="Agent not found",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(property="success", type="boolean"),
     *             @OA\Property(property="message", type="string")
     *         )
     *     ),
     *
     *     @OA\Response(
     *         response=500,
     *         description="Internal Server Error",
     *
     *         @OA\JsonContent(
     *
     *             @OA\Property(property="success", type="boolean"),
     *             @OA\Property(property="message", type="string")
     *         )
     *     ),
     *
     *     security={{"bearerAuth":{}}}
     * )
     */
    public function destroy($id)
    {
        try {
            $success = $this->agentService->deleteAgent($id);

            if (! $success) {
                return response()->json(['success' => false, 'message' => 'Agent not found'], 404);
            }

            return response()->json(['success' => true, 'message' => 'Agent deleted successfully']);
        } catch (Exception $e) {
            return response()->json(['success' => false, 'message' => $e->getMessage()], 500);
        }
    }
}
