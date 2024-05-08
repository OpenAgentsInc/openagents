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
        // Only grab the latest 2 agents
        $this->agents = Agent::latest()->take(2)->get();
    }

    public function render()
    {
        return view('livewire.store')
            ->layout('components.layouts.store');
    }
}
