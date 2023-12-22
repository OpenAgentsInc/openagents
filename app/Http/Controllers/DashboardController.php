<?php

namespace App\Http\Controllers;

use App\Models\Agent;
use App\Models\Run;
use App\Models\Step;
use App\Models\Task;
use Inertia\Inertia;

class DashboardController extends Controller
{
    public function index()
    {
        // Loop through all runs and make an array of only runs belonging to this user's agents. TODO: refactor this
        $userAgents = auth()->user()->agents()->get();
        $allRuns = Run::all();
        $userRuns = [];
        foreach ($allRuns as $run) {
            foreach ($userAgents as $agent) {
                if ($run->agent_id == $agent->id) {
                    array_push($userRuns, $run);
                }
            }
        }

        return Inertia::render('Dashboard', [
            'agents' => Agent::all()->load('tasks'),
            'runs' => $userRuns,
        ]);
    }

    public function referrals()
    {
        return Inertia::render('Referrals', [
            'referrals' => auth()->user()->referrals()->get(),
        ]);
    }
}
