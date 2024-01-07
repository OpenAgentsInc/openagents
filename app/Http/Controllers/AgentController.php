<?php

namespace App\Http\Controllers;

use App\Models\Agent;
use App\Services\Faerie;
use Inertia\Inertia;
use Inertia\Response;

class AgentController extends Controller
{
    // Create a new agent
    public function store()
    {
        request()->validate([
            'name' => 'required',
            'description' => 'required',
            'instructions' => 'required',
            'welcome_message' => 'required'
        ]);

        $name = request('name');
        $description = request('description');
        $instructions = request('instructions');
        $welcome_message = request('welcome_message');

        $agent = Agent::create([
            'user_id' => auth()->user()->id,
            'name' => $name,
            'description' => $description,
            'instructions' => $instructions,
            'welcome_message' => $welcome_message,
        ]);

        return to_route('agent', ['id' => $agent->id])->with('success', 'Agent created!');
    }

    // Show the agent page
    public function show($id)
    {
        try {
            $agent = Agent::findOrFail($id)->load('tasks.steps')->load('brains.datapoints');
            return Inertia::render('AgentView', [
                'agent' => $agent,
            ]);
        } catch (\Exception $e) {
            return redirect('/');
        }
    }

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
}
