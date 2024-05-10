<?php

namespace App\Livewire;

use Livewire\Component;

class Store extends Component
{
    public $agents;

    public function mount()
    {
        //        $this->agents = Agent::where('featured', true)->get();
    }

    public function render()
    {
        return view('livewire.store');
    }
}
