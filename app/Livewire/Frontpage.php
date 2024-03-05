<?php

namespace App\Livewire;

use Livewire\Component;

class Frontpage extends Component
{
    public string $body;

    public function sendMessage()
    {
        $this->validate([
            'body' => 'required|string|max:255',
        ]);

        dd($this->body);

        // Send message
        $this->reset('body');
    }

    public function render()
    {
        return view('livewire.frontpage');
    }
}
