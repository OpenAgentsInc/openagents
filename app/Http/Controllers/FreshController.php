<?php

namespace App\Http\Controllers;

use App\Models\Thread;
use Illuminate\Http\Request;

class FreshController extends Controller
{
    public function fresh()
    {
        $threads = Thread::where('user_id', auth()->id())->get();
        return view('fresh', compact('threads'));
    }

    public function loadChatMessages(Thread $thread)
    {
        $messages = $thread->messages()->orderBy('created_at', 'asc')->get();
        return view('partials.chat_messages', compact('messages'));
    }
}