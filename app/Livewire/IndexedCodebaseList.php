<?php

namespace App\Livewire;

use Livewire\Component;

class IndexedCodebaseList extends Component
{
    public function mount()
    {
        // If user is not logged in or is not pro, redirect to login page
        if (! auth()->check() || ! auth()->user()->isPro()) {
            return redirect('/');
        }
    }

    public function render()
    {
        return view('livewire.indexed-codebase-list');
    }
}
