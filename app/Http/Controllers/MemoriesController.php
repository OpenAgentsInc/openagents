<?php

namespace App\Http\Controllers;

use App\Models\Memory;
use Illuminate\Http\Request;

class MemoriesController extends Controller
{
    public function store(Request $request)
    {
        $memory = Memory::create($request->validated());
        return response()->json($memory, 201);
    }

    public function show($id)
    {
        $memory = Memory::findOrFail($id);
        return response()->json($memory);
    }

    public function update(Request $request, $id)
    {
        $memory = Memory::findOrFail($id);
        $memory->update($request->validated());
        return response()->json($memory);
    }

    public function destroy($id)
    {
        $memory = Memory::findOrFail($id);
        $memory->delete();
        return response()->json(null, 204);
    }

    public function index()
    {
        return response()->json(Memory::all());
    }
}