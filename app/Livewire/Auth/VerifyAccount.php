<?php

namespace App\Livewire\Auth;

use Livewire\Component;

use Illuminate\Auth\Events\Verified;
use Illuminate\Support\Facades\Auth;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use Illuminate\Foundation\Auth\EmailVerificationRequest;

class VerifyAccount extends Component
{

    use LivewireAlert;

    public string $hash;

    public string $id_v;

    public function mount($id,$hash)
    {
        $this->id_v = $id;
        $this->hash = $hash;

        if($this->check()){
            $this->fulfill();
        }else{
            abort('404','Invalid verification link');
        }
    }

    public function check(){
        $user = auth()->user();
        if (! hash_equals((string)  $user->getKey(), (string) $this->id_v)) {
            return false;
        }

        if (! hash_equals(sha1( $user->getEmailForVerification()), (string) $this->hash)) {
            return false;
        }

        return true;
    }





    /**
     * Fulfill the email verification request.
     *
     * @return void
     */
    public function fulfill()
    {
        $user = auth()->user();
        if (! $user->hasVerifiedEmail()) {
            $user->markEmailAsVerified();

            event(new Verified($user));
        }
    }

    public function render()
    {
        return view('livewire.auth.verify-account')
        ->layout('components.layouts.nobar');
    }
}
