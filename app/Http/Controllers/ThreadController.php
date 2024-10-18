<?php

namespace App\Http\Controllers;

use App\Models\Thread;
use App\Models\Message;
use Illuminate\Http\Request;

class ThreadController extends Controller
{
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
}