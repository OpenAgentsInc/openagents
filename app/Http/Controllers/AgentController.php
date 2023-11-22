<?php


namespace App\Http\Controllers;

use App\Models\Agent;
use Inertia\Inertia;
use Inertia\Response;

class AgentController extends Controller
{
  public function store() {
    request()->validate([
      'name' => 'required',
    ]);

    $name = request('name');
// create agent in database
$agent = Agent::create([
    'user_id' => auth()->user()->id,
    'name' => $name,
]);

return response()->json([
    'name' => $agent->name, // changed to return the name of the created agent instead of the passed name variable
], 201);
}
}
