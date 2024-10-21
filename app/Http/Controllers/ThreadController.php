<?php

namespace App\Http\Controllers;

use App\Models\Thread;
use App\Models\Team;
use App\Models\Project;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;

class ThreadController extends Controller
{
    public function index(Request $request)
    {
        $request->validate([
            'team_id' => 'required|exists:teams,id',
            'project_id' => 'nullable|exists:projects,id',
        ]);

        $user = Auth::user();
        $team = Team::findOrFail($request->team_id);

        if (!$user->teams->contains($team)) {
            return response()->json(['error' => 'Unauthorized'], 403);
        }

        $query = Thread::query();

        if ($request->project_id) {
            $project = Project::findOrFail($request->project_id);
            if ($project->team_id !== $team->id) {
                return response()->json(['error' => 'Project does not belong to the specified team'], 400);
            }
            $query->where('project_id', $project->id);
        } else {
            $query->whereIn('project_id', $team->projects->pluck('id'));
        }

        $threads = $query->latest()->paginate(15);

        return view('partials.thread-list', compact('threads'));
    }
}