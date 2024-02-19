<?php

namespace App\Livewire;

use Livewire\Component;
use Illuminate\Support\Facades\Hash;
use Illuminate\Validation\Rules\Password;
use Illuminate\Validation\ValidationException;

class CreatePassword extends Component
{
    public $password;
    public $password_confirmation;

    public function submit()
    {
        $this->validate([
            'password' => ['required', 'min:8'],
            'password_confirmation' => ['required'],
        ], [
            'password.required' => 'A password is required.',
            'password.min' => 'Password must be at least 8 characters.', // Custom message for password length
            'password_confirmation.required' => 'Password confirmation is required.',
        ]);

        if ($this->password !== $this->password_confirmation) {
            throw ValidationException::withMessages([
                'password_confirmation' => ['Passwords must match.'],
            ]);
        }

        dd("valid");
        // Assuming you have a way to get the current user, such as auth()->user()
        // Update the user's password
        // auth()->user()->update([
        //     'password' => Hash::make($this->password),
        // ]);

        // Redirect the user or show a success message
        // session()->flash('status', 'password-updated');
        // return redirect()->to('/home'); // Adjust the redirect as needed
    }

    public function render()
    {
        return view('livewire.create-password')->layout('layouts.blank');
    }
}
