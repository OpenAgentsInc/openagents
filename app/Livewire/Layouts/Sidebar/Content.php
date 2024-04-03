<?php

namespace App\Livewire\Layouts\Sidebar;

use Livewire\Component;

class Content extends Component
{
    public $threads;

    public function mount()
    {
        if (auth()->guest()) {
            $this->threads = [];
        } else {
            $this->threads = auth()->user()->threads;
        }
    }

    public function render()
    {
        return view('livewire.sidebar-content');
    }
}
