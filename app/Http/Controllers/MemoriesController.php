<?php

namespace App\Http\Controllers;

use App\Models\Memory;
use Illuminate\Http\Request;

class MemoriesController extends Controller
{
    public function create(Request $request)
    {
        $memory = Memory::create([
            'title' => $request->title,
            'description' => $request->description,
            'date' => $request->date,
            'location' => $request->location,
            'image' => $request->image,
        ]);

        return response()->json($memory, 201);
    }

    public function read(int $memoryId)
    {
        $memory = Memory::find($memoryId);

        if (!$memory) {
            return response()->json(['message' => 'Memory not found'], 404);
        }

        return response()->json($memory, 200);
    }

    public function update(Request $request, int $memoryId)
    {
        $memory = Memory::find($memoryId);

        if (!$memory) {
            return response()->json(['message' => 'Memory not found'], 404);
        }

        $memory->update([
            'title' => $request->title,
            'description' => $request->description,
            'date' => $request->date,
            'location' => $request->location,
            'image' => $request->image,
        ]);

        return response()->json($memory, 200);
    }

    public function delete(int $memoryId)
    {
        $memory = Memory::find($memoryId);

        if (!$memory) {
            return response()->json(['message' => 'Memory not found'], 404);
        }

        $memory->delete();

        return response()->json(['message' => 'Memory deleted'], 200);
    }
}