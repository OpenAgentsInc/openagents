<?php


namespace App\Http\Controllers;

use App\Models\Agent;
use Inertia\Inertia;
use Inertia\Response;

class ConversationController extends Controller
{
  public function store() {
    request()->validate([
      'agent_id' => 'required',
    ]);
// Given we have an authenticated user, find the agent associated with the given ID
$agent = Agent::find(request('agent_id'));

// Create a conversation between the user and the agent
$request->user()->conversations()->create([
  'agent_id' => $agent->id,
]);
return response()->json([], 201);
  }
}
