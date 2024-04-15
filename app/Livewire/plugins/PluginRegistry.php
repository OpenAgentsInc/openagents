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
                'name' => 'RSS Feed Reader',
                'description' => 'A simple example of a plugin',
                'author' => 'Daniel Ofosu',
            ],
            [
                'name' => 'ZIP Code',
                'description' => 'A plugin from a company',
                'author' => 'McDonald Aladi',
            ],
            [
                'name' => 'Nostr Plugin',
                'description' => 'A plugin from someone else',
                'author' => 'Riccardo Balbo',
            ],
        ];


    }


    public function render()
    {
        return view('livewire.plugins.plugin-registry');

    }
}