<?php

namespace App\Livewire\Auth;

use LivewireUI\Modal\ModalComponent;

class ForgetPassword extends ModalComponent
{
    public bool $show = false;

    // Toggle the value of $show
    public function sendResetLink()
    {

        $this->show = ! $this->show;
    }

    public function render()
    {
        return view('livewire.auth.forget-password');
    }
}
