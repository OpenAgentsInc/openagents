<?php

namespace App\Livewire\Layouts\Sidebar;

use App\Models\Thread;
use Livewire\Component;

class Content extends Component
{
    public $threads;

    public function mount()
    {
        // Set this user's threads
        if (auth()->guest()) {

            $thread = Thread::create([
                'title' => 'Welcome to the forum!',
            ]);

            $this->threads = [$thread];

            return;
        }

        $this->threads = auth()->user()->threads;
    }

    public function render()
    {
        return view('livewire.sidebar-content');
    }
}
