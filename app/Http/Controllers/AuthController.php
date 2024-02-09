<?php

namespace App\Http\Controllers;

use App\Models\User;
use Laravel\Socialite\Facades\Socialite;

class AuthController extends Controller
{
    public function loginGithub()
    {
        return Socialite::driver('github')->redirect();
    }

    public function loginTwitter()
    {
        return Socialite::driver('twitter')->redirect();
    }

    public function twitterCallback()
    {
        $twitterUser = Socialite::driver('twitter')->user();

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

        return redirect('/agents');
    }

    public function githubCallback()
    {
        $githubUser = Socialite::driver('github')->user();

        $user = User::updateOrCreate(
            ['github_id' => $githubUser->id], // Check if GitHub ID exists
            [
                'name' => $githubUser->name,
                'email' => $githubUser->email,
                'github_nickname' => $githubUser->nickname,
                'github_avatar' => $githubUser->avatar,
            ]
        );

        // Log in this user
        auth()->login($user, true);

        return redirect('/agents');
    }

    public function logout()
    {
        auth()->logout();

        return redirect('/');
    }
}
