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
        $socialUser = Socialite::driver('twitter')->user();
        // Check if user already exists in your database based on their email
        $user = User::where('email', $socialUser->email)->first();

        if (! $user) {
            // User doesn't exist, so we create a new user
            $user = new User;
            $user->email = $socialUser->email;
            $user->name = $socialUser->name; // Use Socialite name for user's display name

            // Attempt to set a unique username from the social provider's nickname
            $baseUsername = $socialUser->nickname;
            $username = $baseUsername;
            $counter = 1;
            while (User::where('username', $username)->exists()) {
                // Append number to make username unique
                $username = $baseUsername.$counter;
                $counter++;
            }
            $user->username = $username;

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
