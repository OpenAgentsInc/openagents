<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use App\Models\Memory;

class MemoriesController extends Controller
{
    public function create()
    {
        return view('memories.create');
    }

    public function store(Request $request)
    {
        $memory = new Memory();
        $memory->title = $request->title;
        $memory->description = $request->description;
        $memory->save();

        return redirect()->route('memories.show', ['id' => $memory->id]);
    }

    public function show($id)
    {
        $memory = Memory::findOrFail($id);
        return view('memories.show', ['memory' => $memory]);
    }

    public function update(Request $request, $id)
    {
        $memory = Memory::findOrFail($id);
        $memory->title = $request->title;
        $memory->description = $request->description;
        $memory->save();

        return redirect()->route('memories.show', ['id' => $memory->id]);
    }

    public function destroy($id)
    {
        $memory = Memory::findOrFail($id);
        $memory->delete();

        return redirect()->route('memories.index');
    }
}