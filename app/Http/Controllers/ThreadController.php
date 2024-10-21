<?php

namespace App\Http\Controllers;

use App\Models\Thread;
use App\Models\Team;
use App\Models\Project;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;

class ThreadController extends Controller
{
    public function index(Request $request)
    {
        $user = Auth::user();

        if (!$user->currentTeam) {
            return view('partials.thread-list', ['threads' => collect(), 'message' => 'No team selected. Please create or join a team to see chats.']);
        }

        $request->validate([
            'team_id' => 'required|exists:teams,id',
            'project_id' => 'nullable|exists:projects,id',
        ]);

        $team = Team::findOrFail($request->team_id);

        if (!$user->teams->contains($team)) {
            return response()->json(['error' => 'Unauthorized'], 403);
        }

        $query = Thread::query();

        if ($request->project_id) {
            $project = Project::findOrFail($request->project_id);
            if ($project->team_id !== $team->id) {
                return response()->json(['error' => 'Project does not belong to the specified team'], 400);
            }
            $query->where('project_id', $project->id);
        } else {
            $query->whereIn('project_id', $team->projects->pluck('id'));
        }

        $threads = $query->latest()->paginate(15);

        return view('partials.thread-list', compact('threads'));
    }

    public function messages(Thread $thread)
    {
        $messages = $thread->messages()->orderBy('created_at', 'asc')->get();
        return view('partials.message-list', compact('messages', 'thread'));
    }

    public function show(Thread $thread)
    {
        $messages = $thread->messages()->orderBy('created_at', 'asc')->get();
        return view('chat.show', compact('thread', 'messages'));
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

        if (!$team) {
            return response()->json(['error' => 'No team selected'], 400);
        }

        $thread = new Thread();
        $thread->user_id = $user->id;
        $thread->team_id = $team->id;
        $thread->project_id = $project ? $project->id : null;
        $thread->title = 'New Chat';
        $thread->save();

        $threads = Thread::where('team_id', $team->id)
                         ->when($project, function ($query) use ($project) {
                             return $query->where('project_id', $project->id);
                         })
                         ->latest()
                         ->get();

        return view('partials.thread-list', compact('threads'));
    }
}