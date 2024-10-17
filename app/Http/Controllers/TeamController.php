<?php

namespace App\Http\Controllers;

use App\Models\Team;
use Illuminate\Http\Request;

class TeamController extends Controller
{
    public function threads(Team $team)
    {
        $threads = $team->threads()->get();
        return response()->json($threads, 200);
    }
}