<?php

namespace App\Http\Controllers;

use App\Models\Message;
use App\Models\Thread;
use App\Models\Project;
use Illuminate\Http\Request;

class MessageController extends Controller
{
    public function store(Request $request)
    {
        $validatedData = $request->validate([
            'thread_id' => 'required|exists:threads,id',
            'content' => 'required|string',
        ]);

        $message = Message::create([
            'thread_id' => $validatedData['thread_id'],
            'user_id' => auth()->id(),
            'content' => $validatedData['content'],
        ]);

        return response()->json($message, 201);
    }

    public function storeInThread(Request $request, Thread $thread)
    {
        $validatedData = $request->validate([
            'content' => 'required|string',
        ]);

        $message = $thread->messages()->create([
            'user_id' => $request->input('user_id', auth()->id()),
            'content' => $validatedData['content'],
        ]);

        return response()->json($message, 201);
    }

    public function sendMessage(Request $request)
    {
        $request->validate([
            'message' => 'required|string|max:1000',
            'project_id' => 'nullable|exists:projects,id',
        ]);

        $thread = null;
        if ($request->has('thread_id')) {
            $thread = Thread::findOrFail($request->thread_id);
        } else {
            $thread = new Thread();
            $thread->user_id = auth()->id();
            $thread->title = substr($request->message, 0, 50) . '...';
            
            if ($request->has('project_id')) {
                $project = Project::findOrFail($request->project_id);
                $thread->project_id = $project->id;
            }
            
            $thread->save();
        }

        $message = new Message();
        $message->user_id = auth()->id();
        $message->thread_id = $thread->id;
        $message->content = $request->message;
        $message->is_system_message = false;
        $message->save();

        return redirect()->back()->with('success', 'Message sent successfully!');
    }
}