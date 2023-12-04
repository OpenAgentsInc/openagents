<?php

namespace App\Http\Controllers;

use App\Models\Agent;
use App\Models\Step;
use App\Models\Task;

class InspectController extends Controller
{
    public function index() {
        return view('inspect', [
            'agents' => Agent::all(),
            'tasks' => Task::all(),
            'steps' => Step::all(),
        ]);
    }

    public function show($id) {
        // Return the inspect-task view with just the task and its steps
        $task = Task::with('steps')->findOrFail($id);
        return view('inspect-task', [
            'task' => $task,
            'steps' => $task->steps,
        ]);
    }

    // public function store() {
    //     request()->validate([
    //         'name' => 'required',
    //     ]);

    //     $name = request('name');

    //     // create agent in database
    //     $agent = Agent::create([
    //         'user_id' => auth()->user()->id,
    //         'name' => $name,
    //     ]);

    //     return response()->json([
    //         'name' => $name,
    //     ], 201);
    // }
}
