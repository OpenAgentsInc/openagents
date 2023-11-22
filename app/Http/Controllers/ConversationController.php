<?php


namespace App\Http\Controllers;

use App\Models\Agent;
use Inertia\Inertia;
use Inertia\Response;

class ConversationController extends Controller
{
public function store() {
    try {
        // Validate the request
        request()->validate([
            'agent_id' => 'required',
        ]);

        // Create a new conversation for the authenticated user
        $conversation = request()->user()->conversations()->create([
            'agent_id' => request('agent_id'),
        ]);

        // Return a success response with the created conversation data
        return response()->json([
            'ok' => true,
            'conversation' => $conversation,
        ], 201);
        
    } catch (\Exception $e) {
        // Log any errors and return an error response
        Log::error('ConversationController:store: $e->getMessage(): ' . print_r($e->getMessage(), true));

        return response()->json([
            'ok' => false,
            'error' => 'Error creating conversation.',
        ], 400);
    }
}
}
