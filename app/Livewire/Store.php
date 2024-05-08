<?php

namespace App\Livewire;

use App\AI\Models;
use App\Models\Agent;
use Livewire\Component;

class Store extends Component
{
    public $models = Models::MODELS;

    public $agents;

    public function mount()
    {
        $this->agents = Agent::all();
    }

    public function render()
    {
        return view('livewire.store')
            ->layout('components.layouts.store');
    }
}
