<?php

namespace App\Livewire\Layouts;

use App\Models\Thread;
use Illuminate\Support\Facades\Session;
use Livewire\Attributes\On;
use Livewire\Component;

class SidebarContent extends Component
{
    public $threads = [];

    public function mount()
    {
        $this->refreshThreads();
    }

    #[On('thread-delete')]
    #[On('thread-update')]
    public function refreshThreads()
    {
        $this->threads = $this->getThreadsForUser();
    }

    public function getThreadsForUser()
    {
        if (auth()->guest()) {
            $sessionId = Session::getId();

            return Thread::whereSessionId($sessionId)->orderBy('created_at', 'desc')->get();
        }

        $threads = auth()->user()->threads()->orderBy('created_at', 'desc')->get();

        return $threads ? $threads : collect();
    }

    #[On('codebase-agent-selected')]
    public function codebaseAgentSelected($id)
    {
        dd('nice'.$id);
    }

    public function render()
    {
        return view('livewire.sidebar-content');
    }
}
