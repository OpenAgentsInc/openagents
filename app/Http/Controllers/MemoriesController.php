<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use App\Models\Memory;
use App\Http\Requests\MemoryRequest;

class MemoriesController extends Controller
{
    public function create(MemoryRequest $request)
    {
        $memory = Memory::create([
            'title' => $request->title,
            'description' => $request->description,
            'date' => $request->date,
            'location' => $request->location,
            'agent_id' => $request->agent_id,
        ]);

        return response()->json($memory, 201);
    }

    public function read($id)
    {
        $memory = Memory::findOrFail($id);

        return response()->json($memory, 200);
    }

    public function update(MemoryRequest $request, $id)
    {
        $memory = Memory::findOrFail($id);

        $memory->update([
            'title' => $request->title,
            'description' => $request->description,
            'date' => $request->date,
            'location' => $request->location,
            'agent_id' => $request->agent_id,
        ]);

        return response()->json($memory, 200);
    }

    public function delete($id)
    {
        $memory = Memory::findOrFail($id);

        $memory->delete();

        return response()->json(null, 204);
    }
}