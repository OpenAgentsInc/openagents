<?php

namespace App\Http\Controllers;

use App\Models\Agent;
use App\Models\Plugin;
use App\Models\Task;
use Illuminate\Http\Request;

class AgentController extends Controller
{
    public function chat()
    {
        $task = Task::where('name', 'Inference with web context')->firstOrFail();
        return view('agent-chat', [
            'task' => $task,
        ]);
    }

    public function coder()
    {
        // Grab the first task with the name "Inference with web context"
        $task = Task::where('name', 'Inference with web context')->firstOrFail();

        return view('agent-connie', [
            'task' => $task,
        ]);
    }

    public function build($id)
    {
        $agent = Agent::findOrFail($id);

        return view('agent-builder', [
            'agent' => $agent,
            'tasks' => $agent->tasks->load('steps'),
            'plugins' => Plugin::all(),
        ]);
    }

    public function index()
    {
        $agents = Agent::all();

        return view('agent-list', [
            'agents' => $agents,
        ]);
    }

    public function create()
    {
        return view('agent-create');
    }

    public function run_task(Request $request, $task_id)
    {
        $task = Task::findOrFail($task_id);
        $agent = $task->agent;

        if (! $agent) {
            return response()->json([
                'ok' => false,
                'error' => 'No agent associated with this task.',
            ], 404);
        }

        $output = $agent->runTask($task, [
            'input' => $request->input('input'),
        ]);

        // Assuming $output is a JSON string, decode it to access the response text
        $outputData = json_decode($output, true);
        // It was encoded twice, lets decode again
        if (is_string($outputData)) {
            $outputData = json_decode($outputData, true);
        }
        $jsonError = json_last_error();

        if ($jsonError !== JSON_ERROR_NONE) {
            dd('JSON decoding error: '.json_last_error_msg());
        }
        if (isset($outputData['choices']) && is_array($outputData['choices'])) {
            if (isset($outputData['choices'][0]) && is_array($outputData['choices'][0])) {
                if (isset($outputData['choices'][0]['message']) && is_array($outputData['choices'][0]['message'])) {
                    if (isset($outputData['choices'][0]['message']['content'])) {
                        $textResponse = $outputData['choices'][0]['message']['content'];
                    } else {
                        dd('Content not set', $outputData['choices'][0]['message']);
                    }
                } else {
                    dd('Message not set or not an array', $outputData['choices'][0]);
                }
            } else {
                dd('First choice not set or not an array', $outputData['choices']);
            }
        } else {
            dd('Choices not set or not an array', $outputData);
        }

        // Extract the text response
        $textResponse = '';
        if (isset($outputData['choices'][0]['message']['content'])) {
            $textResponse = $outputData['choices'][0]['message']['content'];
        } else {
            dump($outputData);
            dd('WASNT SET');
        }

        if ($request->wantsJson()) {
            // If client expects JSON, you might still want to simplify the output here
            return response()->json([
                'ok' => true,
                'textResponse' => $textResponse,
            ]);
        } else {
            // For HTML response, pass only the extracted text response
            return response(view('components.task-response', [
                'textResponse' => $textResponse,
            ]));
        }
    }


    // Create a new agent
    public function store()
    {
        request()->validate([
            'name' => 'required',
            'description' => 'required',
            // 'instructions' => 'required',
            // 'welcome_message' => 'required'
        ]);

        $name = request('name');
        $description = request('description');
        $instructions = request('instructions') ?? 'You are a helpful assistant.';
        $welcome_message = request('welcome_message') ?? 'How can I help?';

        $agent = Agent::create([
            'user_id' => auth()->user()->id,
            'name' => $name,
            'description' => $description,
            'instructions' => $instructions,
            'welcome_message' => $welcome_message,
        ]);

        $agent->createChatTask();

        return redirect()->route('agent.build', ['id' => $agent->id])->with('success', 'Agent created!');
    }

    // Show the agent page
    // public function show($id)
    // {
    //     try {
    //         $agent = Agent::findOrFail($id)
    //             ->load([
    //                 'tasks.steps',
    //                 'brains.datapoints',
    //                 'user' => function ($query) {
    //                     $query->select('id', 'github_nickname', 'twitter_nickname')
    //                         ->addSelect(\DB::raw('COALESCE(github_nickname, twitter_nickname) as username'));
    //                 },
    //             ]);

    //         $owner = $agent->user->username;

    //         $conversation = $agent->getUserConversation();

    //         return view('agent-view', [
    //             'agent' => $agent,
    //             'conversation' => $conversation,
    //             'owner' => $owner,
    //             'files' => $agent->files,
    //         ]);
    //     } catch (\Exception $e) {
    //         return redirect('/');
    //     }
    // }

    public function old_chat($id)
    {
        $input = request('input');
        $agent = Agent::findOrFail($id)->load('tasks.steps')->load('brains.datapoints');

        $conversation = $agent->getUserConversation();

        $task = $agent->getChatTask();

        // Return standard JSON success response
        return response()->json([
            'ok' => true,
            'output' => $task->run(['input' => $input, 'conversation' => $conversation]),
        ]);
    }
}
