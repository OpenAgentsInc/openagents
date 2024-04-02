<?php

namespace App\Livewire\Auth;

use LivewireUI\Modal\ModalComponent;

class Login extends ModalComponent
{
    public bool $show = false;

    // Toggle the value of $show
    public function showLogin()
    {

        $this->show = ! $this->show;
    }

    public function render()
    {
        return view('livewire.auth.login');
    }
}
