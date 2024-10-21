<?php

namespace App\Http\Controllers;

use App\Models\Thread;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;

class ChatController extends Controller
{
    public function index(Request $request)
    {
        $user = Auth::user();
        $query = Thread::where('user_id', $user->id);

        if ($request->has('project_id')) {
            $query->where('project_id', $request->project_id);
        }

        $threads = $query->latest()->paginate(15);
        return view('partials.thread-list', compact('threads'));
    }

    public function show(Request $request, Thread $thread)
    {
        $messages = $thread->messages()->orderBy('created_at', 'asc')->get();

        if ($request->header('HX-Request')) {
            // This is an HTMX request, return only the chat content
            return view('chat.show', compact('thread', 'messages'));
        } else {
            // This is a full page load, return the full layout
            return view('dashboard.main-content', [
                'thread' => $thread,
                'messages' => $messages,
            ]);
        }
    }

    public function send(Request $request, Thread $thread)
    {
        $validatedData = $request->validate([
            'content' => 'required|string',
        ]);

        $message = $thread->messages()->create([
            'user_id' => auth()->id(),
            'content' => $validatedData['content'],
        ]);

        if ($request->header('HX-Request')) {
            return view('partials.message', ['message' => $message]);
        }

        return redirect()->route('chat.show', $thread);
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

        $threads = Thread::where('user_id', $user->id)
            ->latest()
            ->get();

        $threadListHtml = view('partials.thread-list', compact('threads'))->render();
        $chatContentHtml = view('chat.show', ['thread' => $thread, 'messages' => []])->render();

        return response()->json([
            'threadList' => $threadListHtml,
            'chatContent' => $chatContentHtml,
            'url' => route('chat.show', $thread)
        ])->header('HX-Push-Url', route('chat.show', $thread));
    }
}