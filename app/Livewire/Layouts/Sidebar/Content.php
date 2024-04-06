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
        $this->refreshThreads();
    }

    #[On('thread-update')]
    public function refreshThreads()
    {
        $this->threads = $this->getThreadsForUser();
    }

    protected function getThreadsForUser()
    {
        if (auth()->guest()) {
            $sessionId = Session::getId();

            return Thread::whereSessionId($sessionId)->latest()->get()->reverse();
        }

        $threads = auth()->user()->threads;

        return $threads ? $threads->reverse() : collect();
    }

    public function render()
    {
        return view('livewire.sidebar-content');
    }
}
