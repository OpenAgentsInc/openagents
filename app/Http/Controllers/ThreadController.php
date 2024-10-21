<?php

namespace App\Http\Controllers;

use App\Models\Thread;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;

class ThreadController extends Controller
{
    public function index(Request $request)
    {
        $user = Auth::user();
        $query = Thread::where('user_id', $user->id);

        if ($request->has('project_id')) {
            $query->where('project_id', $request->project_id);
        }

        $threads = $query->latest()->get();

        if ($request->header('HX-Request')) {
            return view('partials.thread-list', compact('threads'));
        }

        return view('chat.index', compact('threads'));
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
            $threadListHtml = view('partials.thread-list', compact('threads'))->render();
            $chatContentHtml = view('chat.messages', ['messages' => []])->render();

            return response()->json([
                'threadList' => $threadListHtml,
                'chatContent' => $chatContentHtml,
                'url' => route('chat.show', $thread)
            ])->header('HX-Push-Url', route('chat.show', $thread));
        }

        return redirect()->route('chat.show', $thread);
    }
}