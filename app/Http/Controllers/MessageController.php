<?php


namespace App\Http\Controllers;

use App\Models\Agent;
use Inertia\Inertia;
use Inertia\Response;

class MessageController extends Controller
{
  public function store() {
    request()->validate([
      'body' => 'required',
      'conversation_id' => 'required',
    ]);
// create a message in the given conversation
$message = [
    'body' => request('body'),
    'conversation_id' => request('conversation_id'),
    'sender' => 'user'
];
$request->user()->messages()->create($message);
return response()->json([], 201);
  }
}
