<?php

namespace App\Livewire\Auth;

use App\Models\User;
use Livewire\Component;
use Illuminate\Support\Str;
use Livewire\Attributes\Url;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Hash;
use Illuminate\Contracts\Hashing\Hasher;
use Illuminate\Support\Facades\Password;
use Illuminate\Auth\Events\PasswordReset;
use Jantinnerezo\LivewireAlert\LivewireAlert;

class ChangePassword extends Component
{

    use LivewireAlert;


    public bool $show = false;

    public string $password;
    public string $password_confirmation;

    #[Url]
    public $email = '';

    public $token;

    public function mount($token)
    {
        $this->token = $token;

        $this->verify_token();


    }


    public function verify_token()
    {

        $user = User::where('email',$this->email)->first();
        if(is_null($user)){
            abort(404,'Invalid Token!');
        }

        $updatePassword = Password::tokenExists($user,$this->token);

        if (!$updatePassword) {
            abort(404, 'Invalid token!');
        }
    }



    public function render()
    {
        return view('livewire.auth.change-password')
            ->layout('components.layouts.nobar');
    }

    public function reset_account()
    {
        $this->validate([
            'token' => 'required',
            'email' => 'required|email',
            'password' => 'required|confirmed'
        ]);

        $user = User::where('email', $this->email)->first();

        if (is_null($user)) {
            $this->alert('error', 'Invalid token!');
        } else {
            $status = Password::reset(
                ['email' => $this->email, 'password' => $this->password, 'password_confirmation' => $this->password_confirmation, 'token' => $this->token],
                function ($user) {
                    $user->forceFill([
                        'password' => Hash::make($this->password),
                        'remember_token' => Str::random(60),
                    ])->save();

                    $user->tokens()->delete();

                    event(new PasswordReset($user));
                }
            );

            if ($status == Password::PASSWORD_RESET) {
                // return response([
                //     'message' => 'Password reset successfully'
                // ]);
                $this->alert('success', 'Password reset successfully');
                DB::table('password_reset_tokens')->where(['email' => $this->email])->delete();

                $this->alert('success', 'Password Change!');

                $this->show = true;
            }else{
                $this->alert('error', 'Invalid token!');
            }

        }

    }
}
