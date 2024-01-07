<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Inertia\Inertia;

class BuilderController extends Controller
{
    public function showcase()
    {
        return Inertia::render('Showcase');
    }

    public function builder()
    {
        return Inertia::render('Builder');
    }
}
