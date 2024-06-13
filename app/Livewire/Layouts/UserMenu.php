<?php

namespace App\Livewire\Layouts;

use Jantinnerezo\LivewireAlert\LivewireAlert;
use Livewire\Component;

class UserMenu extends Component
{
    use LivewireAlert;

    public function mount()
    {
    }

    public function render()
    {
        return view('livewire.user-menu');
    }
}
