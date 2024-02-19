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

        // Check if there's a user with this email
        $user = User::where('email', $this->email)->first();

        // If no user exists, show the create password component
        if (! $user) {
            session(['email_for_password_creation' => $this->email]);
            return $this->redirect('/create-password', navigate: true);
        }
    }

    public function render()
    {
        return view('livewire.login')->layout('layouts.blank');
    }
}
