<?php

namespace App\Http\Controllers;

use App\Models\Thread;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;

class ChatController extends Controller
{
    public function index()
    {
        /** @var User $user */
        $user = Auth::user();
        $threads = $user->threads()->latest()->get();
        return view('components.chat.index', compact('threads'));
    }

    public function show(Thread $thread)
    {
        $messages = $thread->messages()->orderBy('created_at', 'desc')->get();
        if (request()->header('HX-Request')) {
            return view('components.chat.messages', compact('messages'));
        }
        return view('components.chat.index', compact('thread', 'messages'));
    }

    public function send(Request $request, Thread $thread)
    {
        $request->validate([
            'content' => 'required|string',
        ]);

        $message = $thread->messages()->create([
            'user_id' => Auth::id(),
            'content' => $request->content,
        ]);

        if (request()->header('HX-Request')) {
            return view('chat.message', compact('message'));
        }

        return redirect()->route('chat.show', $thread);
    }
}
