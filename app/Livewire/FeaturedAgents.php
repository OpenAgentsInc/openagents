<?php

namespace App\Livewire;

use App\Models\Agent;
use Livewire\Component;

class FeaturedAgents extends Component
{
    public $agents;

    public function mount()
    {
        $this->agents = Agent::where('featured', true)->limit(2)->get();
    }

    public function render()
    {
        return view('livewire.featured-agents');
    }
}
