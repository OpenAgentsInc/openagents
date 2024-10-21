<?php

namespace App\Http\Controllers;

use App\Models\User;
use Illuminate\Support\Facades\Auth;

class DashboardController extends Controller
{
    public function index()
    {
        // If user is unauthed, just return the homepage
        if (!Auth::check()) {
            return view('homepage');
        }

        // $this->ensureThread();

        return view('dashboard.dashboard');
    }

    // We make sure there is an active thread
    private function ensureThread()
    {
        if (!session()->has('thread')) {
            /** @var User $user */
            $user = Auth::user();
            $thread = $user->threads()->first();

            // If there's no thread, create one
            if (!$thread) {
                $thread = $user->createThread();
            }

            session(['thread' => $thread]);
        }
    }
}
