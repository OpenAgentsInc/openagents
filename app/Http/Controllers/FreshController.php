<?php

namespace App\Http\Controllers;

use App\Models\Thread;
use App\Models\Message;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;

class FreshController extends Controller
{
    public function fresh()
    {
        $threads = Thread::where('user_id', auth()->id())->get();
        return view('fresh', compact('threads'));
    }

    public function loadChatMessages(Thread $thread)
    {
        if ($thread->user_id !== Auth::id()) {
            abort(403, 'Unauthorized action.');
        }

        $messages = $thread->messages()->orderBy('created_at', 'asc')->get();
        return view('partials.chat_messages', compact('messages'));
    }

    public function sendMessage(Request $request, Thread $thread)
    {
        if ($thread->user_id !== Auth::id()) {
            abort(403, 'Unauthorized action.');
        }

        $validated = $request->validate([
            'content' => 'required|string',
        ]);

        $message = new Message([
            'content' => $validated['content'],
            'user_id' => Auth::id(),
        ]);

        $thread->messages()->save($message);

        return view('partials.chat_messages', ['messages' => [$message]]);
    }
}