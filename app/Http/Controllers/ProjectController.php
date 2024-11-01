<?php

namespace App\Http\Controllers;

use App\Models\Project;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\Log;
use Inertia\Inertia;

class ProjectController extends Controller
{
    public function create()
    {
        $user = Auth::user();
        $teamName = null;
        
        if ($user->current_team_id) {
            $team = $user->teams()->find($user->current_team_id);
            if ($team) {
                $teamName = $team->name;
            }
        }

        return Inertia::render('Projects/Create', [
            'teamName' => $teamName,
        ]);
    }

    public function store(Request $request)
    {
        Log::info('Attempting to create project', [
            'user_id' => Auth::id(),
            'name' => $request->name
        ]);

        try {
            $request->validate([
                'name' => ['required', 'string', 'max:255'],
                'description' => ['required', 'string'],
            ]);
        } catch (\Illuminate\Validation\ValidationException $e) {
            Log::info('Validation failed', [
                'errors' => $e->errors(),
                'validator' => $e->validator->failed()
            ]);
            throw $e;
        }

        $user = Auth::user();
        
        $project = Project::create([
            'name' => $request->name,
            'description' => $request->description,
            'user_id' => $user->current_team_id ? null : $user->id,
            'team_id' => $user->current_team_id,
            'status' => 'active',
        ]);

        Log::info('Project created successfully', [
            'project_id' => $project->id,
            'user_id' => $user->id,
            'team_id' => $user->current_team_id
        ]);

        return redirect()->route('projects.show', $project->id);
    }

    public function show($id)
    {
        $project = Project::with(['team', 'files'])->findOrFail($id);
        
        if (!$project->canBeAccessedBy(Auth::user())) {
            abort(403);
        }

        return Inertia::render('Projects/Show', [
            'project' => $project
        ]);
    }
}