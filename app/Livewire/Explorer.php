<?php

namespace App\Livewire;

use Livewire\Component;

class Explorer extends Component
{
    public function render()
    {
        return view('livewire.explorer')->layout('components.layouts.store');
    }
}
