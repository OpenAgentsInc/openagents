<?php

namespace App\Http\Controllers;

use App\Models\Agent;
use App\Services\Faerie;
use Inertia\Inertia;
use Inertia\Response;

class AgentController extends Controller
{
    public function run() {
        $user = auth()->user();
        if ($user->github_nickname !== 'AtlantisPleb') {
            return response()->json([
                'message' => 'You are not AtlantisPleb',
            ], 403);
        }

        $faerie = new Faerie();
        $issue = $faerie->fetchMostRecentIssue();

        return [
            "issue" => $issue,
        ];
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
