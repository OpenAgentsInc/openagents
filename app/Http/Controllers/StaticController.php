<?php

namespace App\Http\Controllers;

use App\Models\User;
use Inertia\Inertia;

class StaticController extends Controller
{
    public function newsplash()
    {
        return view('splash');
    }

    public function splash()
    {
        return Inertia::render('Splash');
    }

    public function terms()
    {
        return Inertia::render('Terms');
    }

    public function privacy()
    {
        return Inertia::render('Privacy');
    }

    public function epstein()
    {
        return redirect('/agent/2');
    }
}
