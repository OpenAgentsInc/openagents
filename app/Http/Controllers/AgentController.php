<?php

namespace App\Http\Controllers;

use App\Models\Agent;
use App\Models\Task;
use Illuminate\Http\Request;
use Inertia\Inertia;
use Inertia\Response;

class AgentController extends Controller
{
    public function run_task(Request $request, $task_id)
    {
        // Find the specified task by task_id
        $task = Task::findOrFail($task_id);

        // Get the agent associated with the task
        $agent = $task->agent;

        if (!$agent) {
            // Handle the case where no agent is associated with the task
            return response()->json([
                'ok' => false,
                'error' => 'No agent associated with this task.',
            ], 404);
        }

        // Run the task on the agent
        $output = $agent->runTask($task, [
            "input" => $request->input('input')
        ]);

        // Return the output of the task execution
        return response()->json([
            'ok' => true,
            'output' => $output,
        ]);
    }

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
            return view('agent-view', [
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
