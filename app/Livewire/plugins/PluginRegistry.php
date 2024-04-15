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
                'name' => 'Daniel Plugin',
                'description' => 'A simple example of a plugin',
                'author' => 'Daniel Ofosu',
            ],
            [
                'name' => 'McDonald Plugin',
                'description' => 'A plugin from a company',
                'author' => 'McDonald Aladi',
            ],
            [
                'name' => 'Riccardo Plugin',
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