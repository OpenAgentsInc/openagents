<?php

namespace App\Livewire;

use Livewire\Component;

class MessagesRemaining extends Component
{
    public $remaining = 49;

    public function render()
    {
        return view('livewire.messages-remaining');
    }
}
