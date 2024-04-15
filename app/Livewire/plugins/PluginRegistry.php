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
                'name' => 'My Plugin',
                'description' => 'A simple example of a plugin',
                'author' => 'Me',
            ],
            [
                'name' => 'McDonald\'s Plugin',
                'description' => 'A plugin from a company',
                'author' => 'McDonald\'s Corporation',
            ],
            [
                'name' => 'Riccardo\'s Plugin',
                'description' => 'A plugin from someone else',
                'author' => 'Riccardo',
            ],
        ];


    }


    public function render()
    {
        return view('livewire.plugins.plugin-registry');

    }
}