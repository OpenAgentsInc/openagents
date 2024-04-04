<?php

namespace App\Livewire\Auth;

use App\Models\User;
use Illuminate\Support\Facades\Password;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use LivewireUI\Modal\ModalComponent;

class ForgetPassword extends ModalComponent
{
    public bool $show = false;

    public string $email;

    use LivewireAlert;

    public function resetPassword()
    {

        $this->validate([
            'email' => ['required', 'email'],
        ]);

        $user = User::where('email', $this->email)->first();

        if ($user && ! empty($user)) {

            // We will send the password reset link to this user. Once we have attempted
            // to send the link, we will examine the response then see the message we
            // need to show to the user. Finally, we'll send out a proper response.
            $status = Password::sendResetLink([
                'email' => $this->email,
            ]);

            if ($status == Password::RESET_LINK_SENT) {

                $this->show = true;
                $this->alert('success', 'Reset link sent!');
            } else {
                $this->alert('error', 'Reset link sent!');
            }
        }
    }

    public function resendLink()
    {

        $user = User::where('email', $this->email)->first();
        // We will send the password reset link to this user. Once we have attempted
        // to send the link, we will examine the response then see the message we
        // need to show to the user. Finally, we'll send out a proper response.
        // $status = Password::broker()->sendResetLink(['email' => $this->email]);

        if ($user && ! is_null($user)) {
            $token = app('auth.password.broker')->createToken($user);
            $user->sendPasswordResetNotification($token);
            $this->alert('success', 'Reset link sent!');
        } else {
            $this->alert('error', 'Reset Link not sent cause an error occured');
        }
    }

    public function render()
    {
        return view('livewire.auth.forget-password');
    }
}
