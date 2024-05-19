<?php

namespace App\Http\Controllers;

use App\Models\User;

class ProfileController extends Controller
{
    public function show($username)
    {
        $user = User::where('username', $username)->firstOrFail();

        return view('profile', compact('user'));
    }
}
