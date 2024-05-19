<?php

namespace App\Http\Controllers;

use App\Models\Thread;
use Illuminate\Support\Facades\Session;

// An experimental HTMX refactor of our Livewire Chat.php component

class ChatController extends Controller
{
    public function index()
    {
        $threads = $this->getThreadsForUser();

        return view('chat', compact('threads'));
    }

    private function getThreadsForUser()
    {
        if (auth()->guest()) {
            $sessionId = Session::getId();

            return Thread::whereSessionId($sessionId)->orderBy('created_at', 'desc')->get();
        }

        return auth()->user()->threads()->orderBy('created_at', 'desc')->get();
    }
}
