<?php

namespace App\Http\Controllers;

use App\Models\User;

class StatsController extends Controller
{
    public function index()
    {
        return 'placeholder';
        // return Inertia::render('Stats', [
        //     'userCount' => User::count(),
        //     'userBalanceSum' => User::sum('balance'),
        // ]);
    }
}
