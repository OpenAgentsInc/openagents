<?php

namespace App\Http\Controllers;

use App\Models\User;

class StaticController extends Controller
{
    public function splash()
    {
        return view('splash');
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
