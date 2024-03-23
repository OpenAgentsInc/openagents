<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Illuminate\Routing\Controller;
use Illuminate\Support\Str;
use Illuminate\View\View;
use Laravel\Jetstream\Jetstream;

class StaticController extends Controller
{
    public function pro(Request $request)
    {
        $policyFile = Jetstream::localizedMarkdownPath('pro.md');

        return view('policy', [
            'policy' => Str::markdown(file_get_contents($policyFile)),
        ]);
    }

    public function docs(Request $request)
    {
        $policyFile = Jetstream::localizedMarkdownPath('docs.md');

        return view('policy', [
            'policy' => Str::markdown(file_get_contents($policyFile)),
        ]);
    }

    public function launch(Request $request)
    {
        $policyFile = Jetstream::localizedMarkdownPath('launch.md');

        return view('policy', [
            'policy' => Str::markdown(file_get_contents($policyFile)),
        ]);
    }

    /**
     * Show the privacy policy for the application.
     *
     * @return View
     */
    public function privacy(Request $request)
    {
        $policyFile = Jetstream::localizedMarkdownPath('policy.md');

        return view('policy', [
            'policy' => Str::markdown(file_get_contents($policyFile)),
        ]);
    }
}
