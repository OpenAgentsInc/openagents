<?php

namespace App\Livewire;

use LivewireUI\Modal\ModalComponent;

class Login extends ModalComponent
{
    public function render()
    {
        return view('livewire.auth.login');
    }
}
