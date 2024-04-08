<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Illuminate\Routing\Controller;
use Illuminate\Support\Str;
use Laravel\Jetstream\Jetstream;

class StaticController extends Controller
{
    public function blog(Request $request)
    {
        return view('blog');
    }

    public function goodbye(Request $request)
    {
        $policyFile = Jetstream::localizedMarkdownPath('goodbye-chatgpt.md');

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

    public function terms(Request $request)
    {
        $policyFile = Jetstream::localizedMarkdownPath('terms.md');

        return view('policy', [
            'policy' => Str::markdown(file_get_contents($policyFile)),
        ]);
    }

    public function privacy(Request $request)
    {
        $policyFile = Jetstream::localizedMarkdownPath('privacy.md');

        return view('policy', [
            'policy' => Str::markdown(file_get_contents($policyFile)),
        ]);
    }
}
