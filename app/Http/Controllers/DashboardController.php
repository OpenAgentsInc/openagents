<?php

namespace App\Http\Controllers;

use App\Models\Agent;
use App\Models\Run;
use App\Models\Step;
use App\Models\Task;
use Inertia\Inertia;

class DashboardController extends Controller
{
    public function index() {
        return Inertia::render('Dashboard', [
            'agents' => Agent::all()->load('tasks'),
            'runs' => Run::all(),
            'steps' => Step::all(),
            'tasks' => Task::all(),
        ]);
    }
}
