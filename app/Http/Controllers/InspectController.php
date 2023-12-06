<?php

namespace App\Http\Controllers;

use App\Models\Agent;
use App\Models\Run;
use App\Models\Step;
use App\Models\Task;
use Inertia\Inertia;

class InspectController extends Controller
{
    public function index() {
        return Inertia::render('Inspect', [
            'agents' => Agent::all()->load('tasks'),
            'tasks' => Task::all(),
            'steps' => Step::all(),
        ]);
    }

    public function showRun($id) {
        dd($id);
        return Inertia::render('Run', [
            'run' => Run::find($id)
        ]);
    }

    public function showTask($id) {
        // Return the inspect-task view with just the task and its steps
        $task = Task::with('steps')->findOrFail($id);
        return view('inspect-task', [
            'task' => $task,
            'steps' => $task->steps,
        ]);
    }

    public function showStep($id) {
        // Return the inspect-step view with just the step and its input/output
        $step = Step::findOrFail($id);
        return view('inspect-step', [
            'step' => $step,
            'input' => $step->input,
            'output' => $step->output,
        ]);
    }
}
