<?php

namespace App\Livewire\Agents\Partials;

use App\Models\Agent;
use Livewire\Component;

class Card extends Component
{
    public $selectedAgent;

    public $agent;

    public function mount($selectedAgent)
    {
        $this->agent = Agent::find($selectedAgent['id']);

        //        dd($selectedAgent['id']); // This will output the selected agent (if you passed it in from the parent component
        $this->selectedAgent = $selectedAgent;
    }

    public function render()
    {
        return view('livewire.agents.partials.card');
    }
}
