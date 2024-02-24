<?php

namespace App\Livewire;

use App\Models\Agent;
use Livewire\Component;

class AgentShow extends Component
{
    public Agent $agent;

    public function mount($id = null)
    {
        // find Agent only where published
        $this->agent = Agent::where('id', $id)->whereNotNull('published_at')->firstOrFail();
    }

    public function render()
    {
        return view('livewire.agent-show');
    }
}
