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
        // Check if the referrer is from Stripe (using our custom pay domain)
        $isFromStripe = $request->server('HTTP_REFERER') && str_contains($request->server('HTTP_REFERER'), 'pay.openagents.com');

        if (! auth()->check() || (! auth()->user()->isPro() && ! $isFromStripe)) {
            return redirect('/');
        }

        return view('pro');
    }

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
