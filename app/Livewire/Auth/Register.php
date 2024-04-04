<?php

namespace App\Livewire\Auth;

use App\Models\User;
use Livewire\Component;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\Hash;
use LivewireUI\Modal\ModalComponent;
use Illuminate\Auth\Events\Registered;
use Jantinnerezo\LivewireAlert\LivewireAlert;

class Register extends ModalComponent
{
    use LivewireAlert;

    public bool $verification = false;

    public bool $show = false;

    public $email;
    public $password;
    public $password_confirmation;

    // Toggle the value of $show
    public function showpassword()
    {
       $validate =  $this->validate([
            'email' => 'required|string|email:rfc,dns|max:250|unique:users,email',

        ]);

        // Validation passed if the code reaches this point
        $this->show = !$this->show;
    }

    // Toggle the value of $show
    public function create()
    {
        $this->validate([
            'password' => 'required|string|min:8|confirmed',
        ]);

        $user = User::create([
            'email' => $this->email,
            'password' => Hash::make($this->password)
        ]);

        event(new Registered($user));

        $credentials = [
            'email' => $this->email,
            'password' => $this->password,
        ];
        Auth::attempt($credentials);
        // Validation passed if the code reaches this point
        $this->set_verified();

        $this->alert('success','Account created successfully');
    }

    public function set_verified()
    {
        $this->verification = !$this->verification;
    }

     /**
     * Resent verificaiton email to user.
     *
     * @param  \Illuminate\Http\Request  $request
     * @return \Illuminate\Http\Response
     */
    public function resend()
    {
        if (Auth::check()){
            auth()->user()->sendEmailVerificationNotification();
            $this->alert('success','A fresh verification link has been sent to your email address.');
        }else{

        }

    }

    public static function closeModalOnClickAway(): bool
    {

        return  true;
    }

    public function render()
    {
        return view('livewire.auth.register');
    }
}
