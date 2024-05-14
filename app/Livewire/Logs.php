<?php

namespace App\Livewire;

use Livewire\Component;

class Logs extends Component
{
    public function mount()
    {
        // Redirect to the homepage if the user is not an admin
        if (! auth()->check() || ! auth()->user()->isAdmin()) {
            return redirect()->route('home');
        }
    }

    public function render()
    {
        return view('livewire.logs');
    }
}
