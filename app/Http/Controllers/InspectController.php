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
}
