<?php

/**
 * Frontpage
 * Shown to first-time visitors to the OpenAgents homepage
 * Visitor is asked what they want an agent to do
 * That begins an introductory conversation as a Thread
 */

namespace App\Livewire;

use App\Models\Thread;
use Livewire\Component;

class Frontpage extends Component
{
    public string $first_message;

    public function sendFirstMessage()
    {
        $this->validate([
            'first_message' => 'required|string|max:255',
        ]);

        // Create a new Thread
        $thread = Thread::create();
        $thread->messages()->create([
            'body' => $this->first_message,
        ]);

        // Redirect to that chat page
        $this->redirect('/chat/'.$thread->id, navigate: true);
    }

    public function render()
    {
        return view('livewire.frontpage');
    }
}
