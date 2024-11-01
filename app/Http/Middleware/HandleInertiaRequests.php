<?php

namespace App\Http\Middleware;

use App\Models\Project;
use App\Models\Thread;
use Illuminate\Http\Request;
use Inertia\Middleware;

class HandleInertiaRequests extends Middleware
{
    /**
     * The root template that is loaded on the first page visit.
     *
     * @var string
     */
    protected $rootView = 'app';

    /**
     * Determine the current asset version.
     */
    public function version(Request $request): ?string
    {
        return parent::version($request);
    }

    /**
     * Define the props that are shared by default.
     *
     * @return array<string, mixed>
     */
    public function share(Request $request): array
    {
        $user = $request->user();
        
        $threads = [];
        $projects = [];

        if ($user) {
            // Get threads
            $threadQuery = Thread::query();
            
            if ($user->current_team_id) {
                // If in team context, get team threads
                $threadQuery->where('team_id', $user->current_team_id);
            } else {
                // If in personal context, get personal threads
                $threadQuery->where('user_id', $user->id)
                     ->whereNull('team_id');
            }
            
            $threads = $threadQuery->orderBy('created_at', 'desc')->get();

            // Get projects
            $projectQuery = Project::query();
            
            if ($user->current_team_id) {
                // If in team context, get team projects
                $projectQuery->where('team_id', $user->current_team_id);
            } else {
                // If in personal context, get personal projects
                $projectQuery->where('user_id', $user->id)
                     ->whereNull('team_id');
            }
            
            $projects = $projectQuery->orderBy('created_at', 'desc')->get();
        }

        return [
            ...parent::share($request),
            'auth' => [
                'user' => $user,
                'teams' => $user ? $user->teams : [],
                'current_team' => $user ? $user->currentTeam : null,
            ],
            'threads' => $threads,
            'projects' => $projects,
        ];
    }
}