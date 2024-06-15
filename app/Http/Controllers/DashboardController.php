<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Illuminate\Routing\Controller;
use Inertia\Inertia;

class DashboardController extends Controller
{
    public function dashboard () {
        return Inertia::render('Dashboard');
    }

    public function plugin_map () {
        return Inertia::render('PluginMap');
    }

    public function test () {
        return Inertia::render('Welcome', [
            'user' => ['name' => 'Test Man']
        ]);
    }

    public function test2 () {
        return Inertia::render('NavTest');
    }
}
