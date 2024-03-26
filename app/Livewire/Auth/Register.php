<?php

namespace App\Livewire\Auth;

use Livewire\Component;

class Register extends Component
{
    public int $step = 1;

    public function set_step(){
        $this->step = 2;
    }

    public function render()
    {
        return view('livewire.auth.register');
    }
}
