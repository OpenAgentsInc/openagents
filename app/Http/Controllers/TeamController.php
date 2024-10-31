<?php

namespace App\Http\Controllers;

use App\Models\Team;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\Log;
use Illuminate\Validation\Rule;

class TeamController extends Controller
{
    public function store(Request $request)
    {
        Log::info('Attempting to create team', [
            'user_id' => Auth::id(),
            'name' => $request->name
        ]);

        try {
            $request->validate([
                'name' => [
                    'required',
                    'string',
                    'max:255',
                    Rule::unique('teams', 'name')->where(fn ($query) => 
                        $query->whereHas('users', fn ($q) => 
                            $q->where('user_id', Auth::id())
                        )
                    ),
                ],
            ]);
        } catch (\Illuminate\Validation\ValidationException $e) {
            Log::info('Validation failed', [
                'errors' => $e->errors(),
                'validator' => $e->validator->failed()
            ]);
            throw $e;
        }

        $team = Team::create([
            'name' => $request->name,
        ]);

        $user = Auth::user();
        $user->teams()->attach($team);
        
        // Set as current team
        $user->current_team_id = $team->id;
        $user->save();

        Log::info('Team created successfully', [
            'team_id' => $team->id,
            'user_id' => $user->id
        ]);

        return redirect()->back();
    }

    public function switchTeam(Request $request)
    {
        Log::info('Attempting to switch team', [
            'user_id' => Auth::id(),
            'team_id' => $request->team_id
        ]);

        try {
            $request->validate([
                'team_id' => [
                    'nullable',
                    Rule::exists('teams', 'id')->where(fn ($query) => 
                        $query->whereHas('users', fn ($q) => 
                            $q->where('user_id', Auth::id())
                        )
                    ),
                ],
            ]);
        } catch (\Illuminate\Validation\ValidationException $e) {
            Log::info('Team switch validation failed', [
                'errors' => $e->errors()
            ]);
            throw $e;
        }

        $user = Auth::user();
        $oldTeamId = $user->current_team_id;
        $user->current_team_id = $request->team_id;
        $user->save();

        Log::info('Team switched', [
            'user_id' => $user->id,
            'old_team_id' => $oldTeamId,
            'new_team_id' => $user->current_team_id,
            'fresh_user_team_id' => $user->fresh()->current_team_id
        ]);

        return redirect()->back();
    }
}