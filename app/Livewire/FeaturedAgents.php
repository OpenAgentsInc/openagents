<?php

namespace App\Livewire;

use App\AI\Models;
use App\Models\Agent;
use Livewire\Component;

class FeaturedAgents extends Component
{
    public $models = Models::MODELS;

    public $agents;

    public function mount()
    {
        $this->agents = Agent::where('featured', true)->get();
    }

    public function render()
    {
        return view('livewire.featured-agents');
    }
}
