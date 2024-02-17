<?php

namespace App\Livewire;

use App\Models\Plugin;
use Livewire\Component;

class PluginList extends Component
{
    public function render()
    {
        return view('livewire.plugin-list', [
            'plugins' => Plugin::all(),
        ]);
    }
}
