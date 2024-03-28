<?php

namespace App\Livewire\Auth;

use Livewire\Component;

class ChangePassword extends Component
{
    public bool $show = false;

    // Toggle the value of $show
    public function changePassword()
    {

        $this->show = ! $this->show;
    }

    public function render()
    {
        return view('livewire.auth.change-password');
    }
}
