<?php

namespace App\Http\Controllers;

use App\Models\User;
use Inertia\Inertia;

class StatsController extends Controller
{
    public function index()
    {
        return Inertia::render('Stats', [
            'userCount' => User::count(),
            'userBalanceSum' => User::sum('balance'),
        ]);
    }
}
