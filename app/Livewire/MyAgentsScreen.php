<?php

namespace App\Livewire;

use Livewire\Component;

class MyAgentsScreen extends Component
{
    public function mount()
    {
        // If user is unauthed, redirect to home
        if (! auth()->check()) {
            return redirect()->route('home');
        }
    }

    public function render()
    {
        return view('livewire.my-agents-screen');
    }
}
