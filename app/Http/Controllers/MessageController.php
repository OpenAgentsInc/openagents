<?php


namespace App\Http\Controllers;

use App\Models\Agent;

class MessageController extends Controller
{
    public function store()
    {
        request()->validate([
          'body' => 'required',
          'conversation_id' => 'required',
        ]);

        // create a message in the given conversation
        request()->user()->messages()->create([
          'body' => request('body'),
          'conversation_id' => request('conversation_id'),
          'sender' => 'user'
        ]);

        return response()->json([], 201);
    }
}
