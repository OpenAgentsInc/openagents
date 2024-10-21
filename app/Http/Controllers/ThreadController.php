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
        $threads = Thread::where('user_id', $user->id)->latest()->paginate(15);
        return view('partials.thread-list', compact('threads'));
    }

    public function messages(Thread $thread)
    {
        $messages = $thread->messages()->orderBy('created_at', 'asc')->get();
        return view('partials.message-list', compact('messages', 'thread'));
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

    public function addMessage(Request $request, Thread $thread)
    {
        $validatedData = $request->validate([
            'content' => 'required|string',
        ]);

        $message = $thread->messages()->create([
            'user_id' => auth()->id(),
            'content' => $validatedData['content'],
        ]);

        return response()->json($message, 201);
    }

    public function process(Request $request, Thread $thread)
    {
        // Implement the logic for processing the thread with LLM tool calls
        // This is a placeholder implementation
        return response()->json(['success' => true, 'message' => 'Thread processed successfully'], 200);
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
            'url' => route('chat.show', $thread->id)
        ])->header('HX-Push-Url', route('chat.show', $thread->id));
    }
}