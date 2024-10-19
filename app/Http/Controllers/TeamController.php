<?php

namespace App\Http\Controllers;

use App\Models\Team;

class TeamController extends Controller
{
    public function threads(Team $team)
    {
        $threads = $team->threads()->with('project')->get();
        return response()->json($threads, 200);
    }

    public function getTeamsAndProjects()
    {
        // TODO: Fetch actual teams and projects from the database
        $teams = ['OpenAgents', 'Atlantis Ports', 'RoA'];
        $projects = ['Project A', 'Project B', 'Project C'];

        return view('components.sidebar.team-switcher-content', compact('teams', 'projects'));
    }
}
