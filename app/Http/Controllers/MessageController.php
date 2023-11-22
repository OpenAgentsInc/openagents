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

return response()->json($message, 201);
return response()->json([], 201);
  }
}
