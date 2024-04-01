<?php

namespace App\Livewire\Auth;

use LivewireUI\Modal\ModalComponent;

class Login extends ModalComponent
{
    public function render()
    {
        return view('livewire.auth.login');
    }
}
