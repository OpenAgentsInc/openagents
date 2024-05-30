<?php

namespace App\Livewire;

use App\Models\User;
use Livewire\Component;

class AgentBalanceSummary extends Component
{
    public int $satsBalance;

    public function mount()
    {
        /** @var User $user */
        $user = auth()->user();

        // Loop through this user's agent balances and sum up their sats_balances
        $this->satsBalance = $user->agents->sum('sats_balance');
    }

    public function render()
    {
        return view('livewire.agent-balance-summary');
    }
}
