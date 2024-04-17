<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Illuminate\Routing\Controller;
use Illuminate\Support\Str;
use Laravel\Jetstream\Jetstream;

class StaticController extends Controller
{
    public function plugins(Request $request)
    {
        $plugins = [
            [
                'name' => 'RSS Feed Reader Plugin',
                'description' => 'An RSS feed reader extism plugin',
                'author' => 'Daniel Ofosu',
                'url' => 'https://github.com/DJrOf/rss-feed-plugin',
            ],
            [
                'name' => 'US Zip Code Information Plugin',
                'description' => 'The US Zip Code Information plugin retrieves detailed information about a zip code in the United States using the Zippopotam API.',
                'author' => 'McDonald Aladi',
                'url' => 'https://github.com/moneya/plugin-zipcode-finder',
            ],
            [
                'name' => 'Key-Value storage Plugin',
                'description' => 'A Key-Value storage plugin for OpenAgents that is eventually-consistent on nostr using NIP-78.',
                'author' => 'Riccardo Balbo',
                'url' => 'https://github.com/riccardobl/openagents-plugins-kv',
            ],
        ];

        return view('plugins', ['plugins' => $plugins]);
    }

    public function changelog(Request $request)
    {
        return view('changelog');
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
