<?php

namespace App\Http\Controllers\Htmx;

use App\Http\Controllers\Controller;
use App\Models\Thread;
use Illuminate\Support\Facades\Session;

class ThreadController extends Controller
{
    public function index()
    {
        $threads = $this->getThreadsForUser();

        return view('components.htmx.threads-list', compact('threads'));
    }

    private function getThreadsForUser()
    {
        if (auth()->guest()) {
            $sessionId = Session::getId();

            return Thread::whereSessionId($sessionId)->orderBy('created_at', 'desc')->get();
        }

        return auth()->user()->threads()->orderBy('created_at', 'desc')->get();
    }

    public function show($threadId)
    {
        $thread = Thread::findOrFail($threadId);
        $messages = $thread->messages()->orderBy('created_at', 'asc')->get();

        return view('components.htmx.messages-list', compact('thread', 'messages'));
    }
}
