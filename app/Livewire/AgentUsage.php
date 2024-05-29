<?php

namespace App\Livewire;

use Livewire\Component;

class AgentUsage extends Component
{
    public $selectedAgent;

    public $sats_balance;

    public function mount($selectedAgent)
    {
        $this->selectedAgent = $selectedAgent;
        if (auth()->check()) {
            $this->sats_balance = auth()->user()->getSatsBalanceAttribute();
        } else {
            $this->sats_balance = 0;
        }
    }

    public function render()
    {
        return view('livewire.agent-usage');
    }
}
