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
$message = request()->user()->messages()->create([
  'body' => request('body'),
  'conversation_id' => request('conversation_id'),
]);

// set sender based on user role
if (request()->user() instanceof Agent) {
    $message->sender = 'agent';
} else {
    $message->sender = 'user';
}

$message->save();
return response()->json([], 201);
  }
}
