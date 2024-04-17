<?php

namespace App\Livewire\Plugins;

use Livewire\Component;

class PluginRegistry extends Component
{
    public $plugins = [];

    public function mount()
    {
        $this->plugins = [
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

    }

    public function render()
    {
        return view('livewire.plugins.plugin-registry');
    }
}
