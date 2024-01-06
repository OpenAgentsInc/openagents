<?php

namespace App\Http\Controllers;

use App\Models\User;
use Inertia\Inertia;
use Laravel\Socialite\Facades\Socialite;

class AuthController extends Controller
{
    public function login()
    {
        return Inertia::render('Login');
    }

    public function loginGithub()
    {
        return Socialite::driver('github')->redirect();
    }

    public function loginTwitter()
    {
        return Socialite::driver('twitter')->redirect();
    }

    public function githubCallback()
    {
        $twitterUser = Socialite::driver('twitter')->user();
        dd($twitterUser);

        // $user = User::updateOrCreate(
        //     ['github_id' => $githubUser->id], // Check if GitHub ID exists
        //     [
        //         'name' => $githubUser->name,
        //         'email' => $githubUser->email,
        //         'github_nickname' => $githubUser->nickname,
        //         'github_avatar' => $githubUser->avatar,
        //     ]
        // );

        // // Log in this user
        // auth()->login($user, true);

        return redirect('/agents');
    }

    public function logout()
    {
        auth()->logout();
        return redirect('/');
    }
}
