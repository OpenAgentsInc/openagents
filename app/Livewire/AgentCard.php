<?php

namespace App\Livewire;

use Livewire\Component;

class AgentCard extends Component
{
    public $agent;

    public function mount($agent)
    {
        $this->agent = $agent;
    }

    public function render()
    {
        return view('livewire.agent-card');
    }
}
