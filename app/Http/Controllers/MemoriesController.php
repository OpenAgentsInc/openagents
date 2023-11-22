<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use App\Models\Memory;

class MemoriesController extends Controller
{
    public function index()
    {
        return Memory::all();
    }

    public function show($id)
    {
        return Memory::find($id);
    }

    public function store(Request $request)
    {
        return Memory::create($request->all());
    }

    public function update(Request $request, $id)
    {
        $memory = Memory::find($id);
        $memory->update($request->all());
        return $memory;
    }

    public function destroy($id)
    {
        Memory::destroy($id);
        return response()->json(['message' => 'Memory deleted successfully']);
    }
}