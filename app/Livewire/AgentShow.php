<?php

namespace App\Livewire;

use App\Models\Agent;
use Livewire\Component;

class AgentShow extends Component
{
    public Agent $agent;

    public function mount($id = null)
    {
        $this->agent = Agent::findOrFail($id);
    }

    public function render()
    {
        return view('livewire.agent-show');
    }
}
