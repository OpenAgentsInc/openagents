<?php

namespace App\Livewire;

use App\Models\User;
use Livewire\Attributes\Validate;
use Livewire\Component;

class Login extends Component
{

    #[Validate('email|required')]
    public $email = '';

    public function submit()
    {
        $this->validate();

        sleep(1);

        // Check if there's a user with this email
        $user = User::where('email', $this->email)->first();
    }

    public function render()
    {
        return view('livewire.login')->layout('layouts.blank');
    }
}
