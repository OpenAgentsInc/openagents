<?php

namespace App\Livewire;

use App\Models\User;
use Livewire\Component;

class Login extends Component
{
    public $email = '';

    public function submit()
    {
        $this->validate([
            'email' => 'required|email',
        ]);

        sleep(1);

        // Check if there's a user with this email
        $user = User::where('email', $this->email)->first();
    }

    public function render()
    {
        return view('livewire.login')->layout('layouts.blank');
    }
}
