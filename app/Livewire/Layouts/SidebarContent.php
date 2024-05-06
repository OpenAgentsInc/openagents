<?php

namespace App\Livewire\Layouts;

use App\Models\Thread;
use Illuminate\Support\Facades\Session;
use Livewire\Attributes\On;
use Livewire\Component;

class SidebarContent extends Component
{
    public $threads = [];

    public $highlightCodebases = false;

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

    #[On('select-model')]
    public function modelSelected()
    {
        $this->highlightCodebases = false;
    }

    #[On('select-agent')]
    public function agentSelected()
    {
        $this->highlightCodebases = false;
    }

    #[On('codebase-agent-selected')]
    public function codebaseAgentSelected($id)
    {
        $this->highlightCodebases = true;
    }

    public function render()
    {
        return view('livewire.sidebar-content');
    }
}
