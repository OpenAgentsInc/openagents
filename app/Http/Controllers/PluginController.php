<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Inertia\Inertia;

class PluginController extends Controller
{
    public function index()
    {
        return view('plugins');
    }
}
