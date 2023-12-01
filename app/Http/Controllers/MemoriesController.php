<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use App\Memory;

class MemoriesController extends Controller
{
    public function create(Request $request)
    {
        $memory = new Memory;
        $memory->title = $request->title;
        $memory->description = $request->description;
        $memory->save();
return response()->json(['message' => 'Memory created successfully'], 201);
}

    public function read($id)
    {
        $memory = Memory::find($id);

        return response()->json($memory);
    }

    public function update(Request $request, $id)
    {
        $memory = Memory::find($id);
        $memory->title = $request->title;
        $memory->description = $request->description;
        $memory->save();

        return response()->json(['message' => 'Memory updated successfully']);
    }

    public function delete($id)
    {
        $memory = Memory::find($id);
        $memory->delete();

        return response()->json(['message' => 'Memory deleted successfully']);
    }

    public function store(Request $request)
    {
        $memory = new Memory;
        $memory->title = $request->title;
        $memory->description = $request->description;
        $memory->save();
return response()->json(['message' => 'Memory created successfully'], 201);
}

    public function edit(Request $request, $id)
    {
        $memory = Memory::find($id);
        $memory->title = $request->title;
        $memory->description = $request->description;
        $memory->save();

        return response()->json(['message' => 'Memory updated successfully']);
    }
}