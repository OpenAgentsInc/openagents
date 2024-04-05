<?php

namespace App\Http\Controllers;

use App\Models\SocialAccount;
use App\Models\User;
use Illuminate\Routing\Controller;
use Socialite;

class SocialAuthController extends Controller
{
    public function login_x()
    {
        return Socialite::driver('twitter')->redirect();
    }

    public function login_x_callback()
    {
        $socialUser = Socialite::driver('twitter')->user();
        $socialData = [
            'id' => $socialUser->id,
            'nickname' => $socialUser->nickname,
            'name' => $socialUser->name,
            'email' => $socialUser->email,
            'avatar' => $socialUser->avatar,
            // Include other data you might want to store
        ];

        // Logic to associate the social account with a user
        $user = User::firstOrCreate(
            ['email' => $socialUser->email],
            ['name' => $socialUser->name]
        );

        $socialAccount = SocialAccount::updateOrCreate(
            [
                'user_id' => $user->id,
                'provider_id' => $socialUser->id,
                'provider_name' => 'twitter',
            ],
            ['provider_data' => $socialData]
        );

        dd($user, $socialAccount);

        auth()->login($user, true);

        return redirect('/');
    }
}
