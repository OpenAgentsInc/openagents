<?php

namespace App\Http\Controllers;

use App\Models\User;

class StaticController extends Controller
{
    public function agentgraph()
    {
        return view('docs.agentgraph');
    }

    public function splash()
    {
        return view('splash');
    }

    public function hud()
    {
        return view('hud');
    }

    public function design()
    {
        return view('design');
    }

    public function terms()
    {
        return view('terms');
    }

    public function privacy()
    {
        return view('privacy');
    }
}
