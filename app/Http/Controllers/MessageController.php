<?php

namespace App\Http\Controllers;

use App\Models\Message;
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
}