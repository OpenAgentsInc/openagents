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
            $conversation = $agent->getUserConversation();
            return Inertia::render('AgentView', [
                'agent' => $agent,
                'conversation' => $conversation,
            ]);
        } catch (\Exception $e) {
            return redirect('/');
        }
    }

    public function chat($id)
    {
        $input = request('input');

        $agent = Agent::findOrFail($id)->load('tasks.steps');

        // If Agent has no tasks or steps, use the default flow
        if ($agent->tasks->count() == 0 || $agent->tasks->first()->steps->count() == 0) {
            $agentResponse = $agent->chat($input, $agent);
        } else {
            $agentResponse = $agent->run(["input" => $input]);
        }

        // Return standard JSON success response
        return response()->json([
            'ok' => true,
            'output' => $agentResponse
        ]);
    }
}
