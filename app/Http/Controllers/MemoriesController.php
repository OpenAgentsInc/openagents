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

    public function create()
    {
        return view('memories.create');
    }
public function store(Request $request)
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
```

Before:
```
public function update(Request $request, $id)
{
    $memory = Memory::findOrFail($id);

    $memory->update([
        'title' => $request->title,
        'description' => $request->description,
        'date' => $request->date,
        'location' => $request->location,
        'image' => $request->image,
    ]);

    return response()->json($memory, 200);
}
```

After:
```
public function update(Request $request, $id)
{
    $memory = Memory::findOrFail($id);

    $memory->update([
        'title' => $request->title,
        'description' => $request->description,
        'date' => $request->date,
        'location' => $request->location,
        'image' => $request->image,
    ]);

    return response()->json($memory, 200);
}
public function show($id)
    {
        return Memory::findOrFail($id);
    }

    public function edit($id)
    {
        return view('memories.edit', ['memory' => Memory::findOrFail($id)]);
    }

    public function update(Request $request, $id)
    {
        $memory = Memory::findOrFail($id);

        $memory->update([
            'title' => $request->title,
            'description' => $request->description,
            'date' => $request->date,
            'location' => $request->location,
            'image' => $request->image,
        ]);

        return response()->json($memory, 200);
    }

    public function destroy($id)
    {
        $memory = Memory::findOrFail($id);
        $memory->delete();

        return response()->json(null, 204);
    }
}