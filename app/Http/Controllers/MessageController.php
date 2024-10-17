<?php

namespace App\Http\Controllers;

use App\Models\Message;
use App\Models\Thread;
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
        ]);

        $message = new Message();
        $message->user_id = auth()->id();
        $message->content = $request->message;
        $message->is_system_message = false;
        
        // If no thread_id is provided, create a new thread
        if (!$request->has('thread_id')) {
            $thread = Thread::create(['user_id' => auth()->id()]);
            $message->thread_id = $thread->id;
        } else {
            $message->thread_id = $request->thread_id;
        }

        $message->save();

        return redirect()->back()->with('success', 'Message sent successfully!');
    }
}