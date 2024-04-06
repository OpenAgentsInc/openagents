<?php

namespace App\Livewire;

use Livewire\Attributes\On;
use Livewire\Component;

class SidebarThread extends Component
{
    public $active;

    public $thread;

    #[On('active-thread')]
    public function activeThreadHandler($id)
    {
        $this->active = (int) $id === $this->thread->id;
    }

    public function render()
    {
        return view('livewire.sidebar-thread');
    }
}
