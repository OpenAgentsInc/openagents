<?php

namespace App\Http\Controllers;

use App\Models\User;

class StaticController extends Controller
{
    public function splash()
    {
        return view('splash');
    }
}
