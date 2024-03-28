<?php

namespace App\Livewire\Auth;

use Livewire\Component;

class Login extends Component
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
