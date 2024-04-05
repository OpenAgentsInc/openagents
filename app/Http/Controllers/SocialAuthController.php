<?php

namespace App\Http\Controllers;

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
        $twitterUser = Socialite::driver('twitter')->user();

        dd($twitterUser);

        $user = User::updateOrCreate(
            ['twitter_id' => $twitterUser->id], // Check if Twitter ID exists
            [
                'name' => $twitterUser->name,
                'email' => $twitterUser->email,
                'twitter_nickname' => $twitterUser->nickname,
                'twitter_avatar' => $twitterUser->avatar,
            ]
        );

        auth()->login($user, true);

        return redirect('/');
    }
}
