<?php

namespace App\Livewire\Auth;

use Illuminate\Support\Facades\Auth;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use Livewire\Attributes\Validate;
use LivewireUI\Modal\ModalComponent;

class Login extends ModalComponent
{
    use LivewireAlert;

    #[Validate('required|email')]
    public $email;

    #[Validate('required')]
    public $password;

    public function login()
    {
        $this->validate();

        $credentials = [
            'email' => $this->email,
            'password' => $this->password,
        ];

        if (Auth::attempt($credentials)) {

            $this->alert('success', 'You have successfully logged in!');

            return $this->redirectRoute('home', navigate: true);
        }

        $this->alert('warning', 'Invalid credentials!');
    }

    public function render()
    {
        return view('livewire.auth.login');
    }
}
