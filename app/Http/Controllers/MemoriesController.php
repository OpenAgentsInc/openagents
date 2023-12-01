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
        return Memory::create($request->all());
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