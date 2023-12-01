<?php

namespace App\Http\Controllers;

use App\Models\Memory;
use Illuminate\Http\Request;

class MemoriesController extends Controller
{
    public function create(Request $request)
    {
        return Memory::create([
            'description' => $request->input('description'),
            'last_accessed' => null,
        ]);
    }

    public function read(int $id)
    {
        return Memory::findOrFail($id);
    }

    public function update(Request $request, int $id)
    {
        $memory = Memory::findOrFail($id);
        $memory->update([
            'description' => $request->input('description'),
        ]);
        return $memory;
    }

    public function delete(int $id)
    {
        $memory = Memory::findOrFail($id);
        $memory->delete();
        return response()->json(['message' => 'Memory deleted successfully']);
    }
}