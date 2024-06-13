<?php

namespace App\Livewire;

use Livewire\Component;

class AgentCard extends Component
{
    public $agent;

    public $showChatButton = true;

    public function mount($agent, $showChatButton = true)
    {
        $this->agent = $agent;
        $this->showChatButton = $showChatButton;
    }

    public function render()
    {
        return view('livewire.agent-card');
    }
}
