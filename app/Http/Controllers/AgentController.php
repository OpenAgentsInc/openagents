<?php

namespace App\Http\Controllers;

use App\Models\Agent;
use Inertia\Inertia;
use Inertia\Response;

class AgentController extends Controller
{
    public function run() {
        $user = auth()->user();
        // Reject if user nickname is not AtlantisPleb
        if ($user->github_nickname !== 'AtlantisPleb') {
            return response()->json([
                'message' => 'You are not AtlantisPleb',
            ], 403);
        }

        return [];
    }

  public function store() {
    request()->validate([
      'name' => 'required',
    ]);

    $name = request('name');

    // create agent in database
    $agent = Agent::create([
      'user_id' => auth()->user()->id,
      'name' => $name,
    ]);

    return response()->json([
      'name' => $name,
    ], 201);
  }
}
