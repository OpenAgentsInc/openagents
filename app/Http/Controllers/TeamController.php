<?php

namespace App\Http\Controllers;

use App\Models\Team;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;
use Illuminate\Validation\Rule;

class TeamController extends Controller
{
    public function store(Request $request)
    {
        $validated = $request->validate([
            'name' => [
                'required',
                'string',
                'max:255',
                Rule::unique('teams')->where(function ($query) {
                    return $query->whereHas('users', function ($q) {
                        $q->where('user_id', Auth::id());
                    });
                }),
            ],
        ]);

        $team = Team::create($validated);

        $user = Auth::user();
        $user->teams()->attach($team);
        
        // Set as current team
        $user->current_team_id = $team->id;
        $user->save();

        return redirect()->back();
    }

    public function switchTeam(Request $request)
    {
        $validated = $request->validate([
            'team_id' => [
                'nullable',
                Rule::exists('teams', 'id')->where(function ($query) {
                    return $query->whereHas('users', function ($q) {
                        $q->where('user_id', Auth::id());
                    });
                }),
            ],
        ]);

        $user = Auth::user();
        
        // If team_id is null or the team exists and user belongs to it
        if (is_null($validated['team_id']) || 
            $user->teams()->where('teams.id', $validated['team_id'])->exists()) {
            $user->current_team_id = $validated['team_id'];
            $user->save();
        }

        return redirect()->back();
    }
}