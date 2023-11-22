
<?php

namespace App\Http\Controllers;

use App\Models\Conversation;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\Request;

class ConversationController extends Controller
{
    /**
     * Store a new conversation.
     *
     * @return \Illuminate\Http\JsonResponse
     */
    public function store()
    {
        // Validate the request data
        Request::validate([
            'agent_id' => 'required',
        ]);

        // Get the authenticated user
        $user = Auth::user();

        // Create a new conversation for the user and agent
        $conversation = $user->conversations()->create([
            'agent_id' => Request::get('agent_id'),
        ]);

        // Return a success response with the created conversation ID
        return response()->json(['conversation_id' => $conversation->id], 201);
    }
}
