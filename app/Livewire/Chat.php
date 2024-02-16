<?php

namespace App\Livewire;

use Livewire\Component;

class Chat extends Component
{
    public $body = '';

    public function sendMessage()
    {
        // Send the message

        // Clear the input
        $this->body = '';
    }

    public function render()
    {
        return view('livewire.chat')->layout('components.layouts.chat');
    }
}
