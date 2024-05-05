<?php

namespace App\Livewire;

use App\Models\Codebase;
use Livewire\Component;

class IndexedCodebaseList extends Component
{
    public $codebases = [];

    public function mount()
    {
        // If user is not logged in or is not pro, redirect to login page
        if (! auth()->check() || ! auth()->user()->isPro()) {
            return redirect('/');
        }

        $this->codebases = Codebase::all()->toArray();
    }

    public function render()
    {
        return view('livewire.indexed-codebase-list');
    }
}
