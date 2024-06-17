<?php

namespace App\Http\Controllers;

use App\Models\Thread;
use App\Models\User;
use Illuminate\Routing\Controller;
use Illuminate\Support\Facades\Session;
use Laravel\Socialite\Facades\Socialite;

class SocialAuthController extends Controller
{
    public function login_x()
    {
        return Socialite::driver('twitter')->redirect();
    }

    public function login_x_callback()
    {
        // If query string 'denied' exists, redirect to homepage
        if (request()->query('denied')) {
            return redirect('/');
        }

        $socialUser = Socialite::driver('twitter')->user();
        // Check if user already exists in your database based on their email
        $alternativeEmail = $socialUser->nickname.'@fakeemail.com';
        $user = User::where('email', $socialUser->email)->orWhere('email', $alternativeEmail)->first();
        $sessionId = Session::getId(); // Get the current session ID

        if (! $user) {
            // User doesn't exist, so we create a new user

            // Check if email is null or empty string, if so set it to the username plus @fakeemail.com
            if (empty($socialUser->email)) {
                $email = $socialUser->nickname.'@fakeemail.com';
            } else {
                $email = $socialUser->email;
            }

            $username = $socialUser->nickname;
            if (strpos($username, 'npub') === 0) {
                $username = 'x-'.$username;
            }

            $user = User::create([
                'email' => $email,
                'name' => $socialUser->name,
                'username' => $username,
                'external_id' => $socialUser->nickname,
                'auth_provider' => 'X',
            ]);

            // Set the profile photo path from the social provider
            if (isset($socialUser->avatar)) {
                $avatarUrl = $socialUser->avatar;
                // Check if the URL starts with 'http://' and replace with 'https://'
                if (strpos($avatarUrl, 'http://') === 0) {
                    $avatarUrl = str_replace('http://', 'https://', $avatarUrl);
                }
                $user->profile_photo_path = $avatarUrl;
            }

            $user->save();

            // Retrieve threads with the current session ID
            $threads = Thread::whereSessionId($sessionId)->get();

            // Update threads with the current session ID to have the new user's ID
            Thread::whereSessionId($sessionId)->update(['user_id' => $user->id]);
        } else {
            // User exists, check if we need to update the profile photo
            if (empty($user->profile_photo_path) && isset($socialUser->avatar)) {
                $user->profile_photo_path = $socialUser->avatar;
                $user->save();
            }
        }

        auth()->login($user, true);

        return redirect('/');
    }
}
