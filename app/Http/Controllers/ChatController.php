<?php

namespace App\Http\Controllers;

use App\Models\Thread;
use Illuminate\Http\RedirectResponse;
use Illuminate\Support\Facades\Auth;
use Inertia\Inertia;
use Inertia\Response;

class ChatController
{
    public function chat(): RedirectResponse|Response
    {
        if (request()->path() === 'chat') {
            // Create a new thread for the user
            $thread = Thread::create([
                'user_id' => Auth::id(),
                'title' => 'New Chat',
            ]);

            return redirect("/chat/{$thread->id}");
        }

        return Inertia::render('Chat');
    }
}