<?php

namespace App\Http\Controllers;

use App\Models\User;

class ProfileController extends Controller
{
    public function show($username)
    {
        $user = User::where('username', $username)->first();

        if (! $user) {
            $user = User::where('name', $username)->firstOrFail();
        }

        return view('profile', compact('user'));
    }
}
