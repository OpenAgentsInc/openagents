<?php

namespace App\Livewire\Layouts;

use App\Models\Thread;
use Illuminate\Support\Facades\Session;
use Livewire\Attributes\On;
use Livewire\Component;

class SidebarContent extends Component
{
    public $threads;

    public $activeThreadId;

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

            return Thread::whereSessionId($sessionId)->orderBy('created_at', 'desc')->get();
        }

        $threads = auth()->user()->threads()->orderBy('created_at', 'desc')->get();

        return $threads ? $threads : collect();
    }

    #[On('active-thread')]
    public function activeThreadHandler($id)
    {
        $this->activeThreadId = $id;
    }

    public function render()
    {
        return view('livewire.sidebar-content');
    }
}
