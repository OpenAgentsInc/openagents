<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use App\Models\Memory;

class MemoriesController extends Controller
{
    public function store(Request $request)
    {
        return Memory::create($request->all());
    }

    public function show($id)
    {
        return Memory::findOrFail($id);
    }

    public function update(Request $request, $id)
    {
        $memory = Memory::findOrFail($id);
        $memory->update($request->all());
        return $memory;
    }

    public function destroy($id)
    {
        $memory = Memory::findOrFail($id);
        $memory->delete();
        return response()->json(null, 204);
    }
}