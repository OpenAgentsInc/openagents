<?php

namespace App\Livewire\Agents\Partials;

use Livewire\Component;

class Card extends Component
{
    public $selectedAgent;

    public function mount($selectedAgent)
    {
        $this->selectedAgent = $selectedAgent;
    }

    public function render()
    {
        return view('livewire.agents.partials.card');
    }
}
