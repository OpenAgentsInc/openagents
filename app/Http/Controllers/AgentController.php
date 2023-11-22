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
  'memory_manager_id' => null, // add new field for memory manager id
]);
return response()->json([
      'name' => $name,
    ], 201);
  }
}
