<?php

namespace App\Livewire\Auth;

use Livewire\Component;

class Register extends Component
{
    public bool $verification = false;

    public bool $show = false;

    // Toggle the value of $show
    public function showpassword()
    {

        $this->show = ! $this->show;
    }

    public function set_verified()
    {
        $this->verification = ! $this->verification;
    }

    public function render()
    {
        return view('livewire.auth.register');
    }
}
