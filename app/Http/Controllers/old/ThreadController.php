<?php

namespace App\Http\Controllers;

use App\Models\Thread;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;
use Illuminate\Database\Eloquent\Collection;

class ThreadController extends Controller
{
    public function index(Request $request)
    {
        $user = Auth::user();

        if (!$user) {
            return response('Unauthorized', 401);
        }

        $query = Thread::where('user_id', $user->id);

        if ($request->has('project_id')) {
            $query->where('project_id', $request->project_id);
        }

        $threads = $query->latest()->get();

        if ($request->header('HX-Request')) {
            return view('components.sidebar.thread-list', compact('threads'));
        }

        return view('components.chat.index', compact('threads'));
    }

    public function create(Request $request)
    {
        $user = Auth::user();
        $team = $user->currentTeam;
        $project = $user->currentProject;

        $thread = new Thread();
        $thread->user_id = $user->id;
        $thread->team_id = $team ? $team->id : null;
        $thread->project_id = $project ? $project->id : null;
        $thread->title = 'New Chat';
        $thread->save();

        if ($request->header('HX-Request')) {
            $threads = Thread::where('user_id', $user->id)->latest()->get();

            $response = view('components.chat.messages', ['messages' => new Collection(), 'thread' => $thread])
                ->render();

            $response .= '
                <div id="thread-list" hx-swap-oob="true">
                    ' . view('components.sidebar.thread-list', compact('threads'))->render() . '
                </div>
                <div id="main-content" hx-swap-oob="true">
                    ' . view('components.chat.index', compact('thread', 'threads'))->render() . '
                </div>
            ';

            return response($response)
                ->header('HX-Push-Url', route('chat.show', $thread));
        }

        return redirect()->route('chat.show', $thread);
    }

    public function show(Thread $thread)
    {
        $messages = $thread->messages()->orderBy('created_at', 'asc')->get();
        $threads = Auth::user()->threads()->latest()->get();

        if (request()->header('HX-Request')) {
            return view('components.chat.messages', compact('thread', 'messages'));
        }

        return view('components.chat.index', compact('thread', 'messages', 'threads'));
    }
}
