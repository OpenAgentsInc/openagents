<?php

namespace App\Http\Controllers;

use App\Models\Thread;
use Illuminate\Http\Client\Request;

class ChatController extends Controller
{
    public function index()
    {
        return "Chat";
    }

    public function show(Thread $thread)
    {
        $messages = $thread->messages()->orderBy('created_at', 'asc')->get();
        return "ok";
        // return view('chat.show', compact('thread', 'messages'));
    }

    public function addMessage(Request $request, Thread $thread)
    {
        return redirect()->route('chat.index');
    }
}
