<?php

namespace App\Livewire;

use Livewire\Component;

class Admin extends Component
{
    public function mount()
    {
        if (! auth()->user() || ! auth()->user()->isAdmin()) {
            return redirect()->route('home');
        }
    }

    public function render()
    {
        return view('livewire.admin')->layout('components.layouts.admin');
    }
}
