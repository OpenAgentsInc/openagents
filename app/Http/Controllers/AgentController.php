<?php

namespace App\Http\Controllers;

use App\Models\Agent;
use App\Services\Faerie;
use Inertia\Inertia;
use Inertia\Response;

class AgentController extends Controller
{
    public function chat($id)
    {
        $input = request('input');

        $agent = Agent::findOrFail($id)->load('tasks.steps');
        $agentResponse = $agent->run(["input" => $input]);

        // Return standard JSON success response
        return response()->json([
            'ok' => true,
            'output' => $agentResponse
        ]);
    }

    public function show($id)
    {
        try {
            $agent = Agent::findOrFail($id)->load('brain')->load('tasks.steps');
            return Inertia::render('AgentNodes', [
                'agent' => $agent,
            ]);
        } catch (\Exception $e) {
            dd($e);
            // redirect to homepage, inertia style
            return \to_route('chat');
        }
    }

    public function run()
    {
        $user = auth()->user();
        if ($user->github_nickname !== 'AtlantisPleb') {
            return response()->json([
                'message' => 'You are not AtlantisPleb',
            ], 403);
        }

        try {
            $faerie = new Faerie();
            $run = $faerie->runJob();
            return $run;
        } catch (\Exception $e) {
            return response()->json([
                'message' => $e->getMessage(),
            ], 500);
        }
    }

    public function store()
    {
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
