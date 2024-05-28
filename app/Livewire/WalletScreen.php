<?php

namespace App\Livewire;

use App\Models\User;
use Livewire\Component;

class WalletScreen extends Component
{
    public $balance_btc = 1;

    // On mount, grab the user's bitcoin balance
    public function mount()
    {
        // If the user is not logged in, redirect to the homepage
        if (! auth()->check()) {
            return redirect()->route('home');
        }

        /** @var User $user */
        $user = auth()->user();
        $this->balance_btc = $user->getSatsBalanceAttribute();
    }

    public function render()
    {
        return view('livewire.wallet-screen');
    }
}
