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

        $agent->createChatTask();

        return redirect()->route('agent', ['id' => $agent->id])->with('success', 'Agent created!');
    }

    // Show the agent page
    public function show($id)
    {
        try {
            $agent = Agent::findOrFail($id)
            ->load([
                'tasks.steps',
                'brains.datapoints',
                'user' => function ($query) {
                    $query->select('id', 'github_nickname', 'twitter_nickname')
                        ->addSelect(\DB::raw('COALESCE(github_nickname, twitter_nickname) as username'));
                },
            ]);

            $owner = $agent->user->username;

            $conversation = $agent->getUserConversation();
            return Inertia::render('AgentView', [
                'agent' => $agent,
                'conversation' => $conversation,
                'owner' => $owner,
                'files' => $agent->files,
            ]);
        } catch (\Exception $e) {
            return redirect('/');
        }
    }

    public function chat($id)
    {
        $input = request('input');
        $agent = Agent::findOrFail($id)->load('tasks.steps')->load('brains.datapoints');

        $conversation = $agent->getUserConversation();

        // If Agent has a brain, use retrieval. Otherwise use default chat task.
        if ($agent->brains->count() > 0) {
            $task = $agent->getRetrievalTask();
        } else {
            $task = $agent->getChatTask();
        }

        // Return standard JSON success response
        return response()->json([
            'ok' => true,
            'output' => $task->run(["input" => $input, "conversation" => $conversation])
        ]);
    }
}
