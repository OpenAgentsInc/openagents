<?php

namespace App\Http\Controllers;

use App\Models\Thread;
use Illuminate\Http\RedirectResponse;
use Illuminate\Support\Facades\Auth;
use Inertia\Inertia;
use Inertia\Response;

class ChatController
{
    public function chat($id = null): RedirectResponse|Response
    {
        if (request()->path() === 'chat') {
            // Create a new thread for the user
            $thread = Thread::create([
                'user_id' => Auth::id(),
                'title' => 'New Chat',
            ]);

            return redirect("/chat/{$thread->id}");
        }

        // Load thread with messages and their tool invocations
        $thread = Thread::with('messages.toolInvocations')->findOrFail($id);

        // Get all threads for the current user
        $threads = Thread::where('user_id', Auth::id())
            ->orderBy('created_at', 'desc')
            ->get();

        return Inertia::render('Chat', [
            'messages' => $thread->messages->map(function ($message) {
                return array_merge($message->toArray(), [
                    'toolInvocations' => $message->toolInvocations
                ]);
            }),
            'currentChatId' => $thread->id,
            'threads' => $threads
        ]);
    }
}