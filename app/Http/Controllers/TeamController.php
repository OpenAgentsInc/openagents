<?php

namespace App\Http\Controllers;

use App\Models\Team;
use App\Models\Project;
use Illuminate\Support\Facades\Auth;

class TeamController extends Controller
{
    public function threads(Team $team)
    {
        $threads = $team->threads()->with('project')->get();
        return response()->json($threads, 200);
    }

    public function getTeamsAndProjects()
    {
        $user = Auth::user();
        $teams = $user->teams()->pluck('name', 'id')->toArray();
        $projects = Project::whereIn('team_id', array_keys($teams))->pluck('name')->toArray();

        return view('components.sidebar.team-switcher-content', compact('teams', 'projects'));
    }
}