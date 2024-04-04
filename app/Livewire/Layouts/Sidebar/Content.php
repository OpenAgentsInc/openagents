<?php

namespace App\Livewire\Layouts\Sidebar;

use App\Models\Thread;
use Livewire\Component;
use Livewire\Attributes\On;

class Content extends Component
{

    public $threads;

    public function mount()
    {
        if (auth()->guest()) {
            // temporary
            $this->threads = Thread::all()->reverse();
        } else {
            $this->threads = auth()->user()->threads;
            if (! $this->threads) {
                $this->threads = [];
            }
        }
    }

    #[On('thread-update')]
    public function refershThread(){
        if (auth()->guest()) {
            // temporary
            $this->threads = Thread::all()->reverse();
        } else {
            $this->threads = auth()->user()->threads;
            if (! $this->threads) {
                $this->threads = [];
            }
        }
    }

    public function render()
    {
        return view('livewire.sidebar-content');
    }
}
