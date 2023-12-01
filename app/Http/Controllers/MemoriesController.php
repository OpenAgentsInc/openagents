<?php

namespace App\Http\Controllers;

use App\Models\Memory;
use Illuminate\Http\Request;

class MemoriesController extends Controller
{
    public function index()
    {
        return Memory::all();
    }
public function store(Request $request)
{
    $memory = Memory::create($request->all());
    return response()->json($memory, 201);
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

        return 204;
    }
}