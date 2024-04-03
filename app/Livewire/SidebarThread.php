<?php

namespace App\Livewire;

use Livewire\Component;

class SidebarThread extends Component
{
    public $thread;

    public function render()
    {
        return view('livewire.sidebar-thread');
    }
}
