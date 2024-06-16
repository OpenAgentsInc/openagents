<?php

namespace App\Http\Controllers;

use Inertia\Inertia;

class AuthController extends Controller {
    public function login_page () {
        return Inertia::render('Login');
    }
}
