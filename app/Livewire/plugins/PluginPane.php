<?php

namespace App\Livewire\Plugins;

use Livewire\Component;

class PluginPane extends Component
{
    public $plugin;

    public function mount($plugin)
    {
        $this->plugin = $plugin;
    }

    public function render()
    {
        return view('livewire.plugins.plugin-pane');
    }
}
