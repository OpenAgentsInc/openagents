<?php

namespace App\Http\Controllers;

use App\Models\Thread;
use App\Models\Message;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\Validator;

class FreshController extends Controller
{
    public function fresh()
    {
        $threads = Thread::where('user_id', auth()->id())->get();
        return view('fresh', compact('threads'));
    }

    public function loadChatMessages(Thread $thread)
    {
        if ($thread->user_id !== Auth::id()) {
            abort(403, 'Unauthorized action.');
        }

        $messages = $thread->messages()->orderBy('created_at', 'asc')->get();
        return view('partials.chat_messages', compact('messages'));
    }

    public function sendMessage(Request $request, Thread $thread)
    {
        if ($thread->user_id !== Auth::id()) {
            abort(403, 'Unauthorized action.');
        }

        $validator = Validator::make($request->all(), [
            'content' => 'required|string',
        ]);

        if ($validator->fails()) {
            if ($request->ajax()) {
                return response()->json(['errors' => $validator->errors()], 422);
            }
            return redirect()->back()->withErrors($validator)->withInput();
        }

        $message = new Message([
            'content' => $validator->validated()['content'],
            'user_id' => Auth::id(),
        ]);

        $thread->messages()->save($message);

        if ($request->ajax()) {
            return view('partials.chat_messages', ['messages' => [$message]]);
        }

        return redirect()->back();
    }
}