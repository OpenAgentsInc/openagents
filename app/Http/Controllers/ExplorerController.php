<?php

namespace App\Http\Controllers;

class ExplorerController extends Controller
{
    public function index()
    {
        return view('explorer.explorer');
    }
}
