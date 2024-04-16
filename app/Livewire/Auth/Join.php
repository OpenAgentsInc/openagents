<?php

namespace App\Livewire\Auth;

use Jantinnerezo\LivewireAlert\LivewireAlert;
use LivewireUI\Modal\ModalComponent;

class Join extends ModalComponent
{
    use LivewireAlert;

    public function render()
    {
        return view('livewire.auth.join');
    }
}
