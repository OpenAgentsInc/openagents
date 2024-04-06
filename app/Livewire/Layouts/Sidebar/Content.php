<?php

namespace App\Livewire\Layouts\Sidebar;

use App\Models\Thread;
use Illuminate\Support\Facades\Session;
use Livewire\Attributes\On;
use Livewire\Component;

class Content extends Component
{
    public $threads;

    public function mount()
    {
        if (auth()->guest()) {
            $sessionId = Session::getId();
            $this->threads = Thread::whereSessionId($sessionId)->latest()->get();
        } else {
            $this->threads = auth()->user()->threads;
            if (! $this->threads) {
                $this->threads = [];
            }
        }
    }

    #[On('thread-update')]
    public function refreshThread()
    {
        if (auth()->guest()) {
            $sessionId = Session::getId();
            $this->threads = Thread::whereSessionId($sessionId)->latest()->get();
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
