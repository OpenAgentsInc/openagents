<?php

namespace App\Livewire\Auth;

use Livewire\Component;

class ForgetPassword extends Component
{
    public bool $show = false;

    // Toggle the value of $show
    public function  sendResetLink(){

        $this->show = !$this->show;
    }

    public function render()
    {
        return view('livewire.auth.forget-password');
    }
}
