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
        $request->validate([
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

        $team = Team::create([
            'name' => $request->name,
        ]);

        $user = Auth::user();
        $user->teams()->attach($team);
        
        // Set as current team
        $user->current_team_id = $team->id;
        $user->save();

        return redirect()->back();
    }

    public function switchTeam(Request $request)
    {
        $request->validate([
            'team_id' => [
                'nullable',
                Rule::exists('teams', 'id')->where(function ($query) {
                    $query->whereHas('users', function ($q) {
                        $q->where('user_id', Auth::id());
                    });
                }),
            ],
        ]);

        $user = Auth::user();
        $user->current_team_id = $request->team_id;
        $user->save();

        return redirect()->back();
    }
}