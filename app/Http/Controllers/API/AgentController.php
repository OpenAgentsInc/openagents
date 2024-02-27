<?php

namespace App\Http\Controllers\API;

use App\Http\Controllers\Controller;
use App\Models\Agent;
use Illuminate\Http\Request;

class AgentController extends Controller
{
    /**
     * Display a listing of the resource.
     */
    public function index()
    {
        //
    }

    /**
     * Store a newly created resource in storage.
     */
    public function store(Request $request)
    {
        // Validate request has name and description
        $request->validate([
            'name' => 'required',
            'description' => 'required',
            'instructions' => 'required',
            'welcome_message' => 'required',
        ]);

        // Create agent
        $agent = Agent::create([
            'name' => $request->name,
            'description' => $request->description,
            'instructions' => $request->instructions,
            'welcome_message' => $request->welcome_message,
            'user_id' => $request->user()->id,
        ]);

        return response()->json($agent, 201);
    }

    /**
     * Display the specified resource.
     */
    public function show(string $id)
    {
        //
    }

    /**
     * Update the specified resource in storage.
     */
    public function update(Request $request, string $id)
    {
        //
    }

    /**
     * Remove the specified resource from storage.
     */
    public function destroy(string $id)
    {
        //
    }
}
