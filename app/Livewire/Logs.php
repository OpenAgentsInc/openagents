<?php

namespace App\Livewire;

use App\Models\Log;
use Livewire\Component;

class Logs extends Component
{
    public $logs = [];

    public function mount()
    {
        // Redirect to the homepage if the user is not an admin
        if (! auth()->check() || ! auth()->user()->isAdmin()) {
            return redirect()->route('home');
        }

        $this->logs = Log::latest()->get();
    }

    public function render()
    {
        return view('livewire.logs');
    }
}
