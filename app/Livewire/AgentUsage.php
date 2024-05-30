<?php

namespace App\Livewire;

use Livewire\Attributes\On;
use Livewire\Component;

class AgentUsage extends Component
{
    public $selectedAgent;

    public $sats_balance;

    public function mount($selectedAgent)
    {
        $this->selectedAgent = $selectedAgent;
        if (auth()->check()) {
            $this->updateSatsBalance();
        } else {
            $this->sats_balance = 0;
        }
    }

    private function updateSatsBalance()
    {
        $this->sats_balance = auth()->user()->getSatsBalanceAttribute();
    }

    #[On('message-created')]
    public function updateStuff()
    {
        $this->updateSatsBalance();
    }

    public function render()
    {
        return view('livewire.agent-usage');
    }
}
