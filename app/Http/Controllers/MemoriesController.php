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
    $memory = Memory::create([
        'title' => $request->title,
        'description' => $request->description,
    ]);

    return response()->json($memory, 201);
}
```

Before:
```
public function update(Request $request, $id)
{
    $memory = Memory::findOrFail($id);
    $memory->title = $request->title;
    $memory->description = $request->description;
    $memory->save();

    return redirect()->route('memories.show', ['id' => $memory->id]);
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
    ]);

    return response()->json($memory, 200);
}
```

Before:
```
public function destroy($id)
{
    $memory = Memory::findOrFail($id);
    $memory->delete();

    return redirect()->route('memories.index');
}
```

After:
```
public function destroy($id)
{
    $memory = Memory::findOrFail($id);
    $memory->delete();

    return response()->json(null, 204);
}
```

Before:
```
public function show($id)
{
    $memory = Memory::findOrFail($id);
    return view('memories.show', ['memory' => $memory]);
}
```

After:
```
public function show($id)
{
    $memory = Memory::findOrFail($id);
    return response()->json($memory, 200);
}
```

Before:
```
public function store(Request $request)
{
    $memory = new Memory();
    $memory->title = $request->title;
    $memory->description = $request->description;
    $memory->save();

    return redirect()->route('memories.show', ['id' => $memory->id]);
}
```

After:
```
public function store(Request $request)
{
    $memory = Memory::create([
        'title' => $request->title,
        'description' => $request->description,
    ]);

    return response()->json($memory, 201);
}
```

Before:
```
public function update(Request $request, $id)
{
    $memory = Memory::findOrFail($id);
    $memory->title = $request->title;
    $memory->description = $request->description;
    $memory->save();

    return redirect()->route('memories.show', ['id' => $memory->id]);
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
    ]);

    return response()->json($memory, 200);
}
```

Before:
```
public function destroy($id)
{
    $memory = Memory::findOrFail($id);
    $memory->delete();

    return redirect()->route('memories.index');
}
```

After:
```
public function destroy($id)
{
    $memory = Memory::findOrFail($id);
    $memory->delete();

    return response()->json(null, 204);
}
```

Before:
```
public function show($id)
{
    $memory = Memory::findOrFail($id);
    return view('memories.show', ['memory' => $memory]);
}
```

After:
```
public function show($id)
{
    $memory = Memory::findOrFail($id);
    return response()->json($memory, 200);
}
```

Before:
```
public function store(Request $request)
{
    $memory = new Memory();
    $memory->title = $request->title;
    $memory->description = $request->description;
    $memory->save();

    return redirect()->route('memories.show', ['id' => $memory->id]);
}
```

After:
```
public function store(Request $request)
{
    $memory = Memory::create([
        'title' => $request->title,
        'description' => $request->description,
    ]);

    return response()->json($memory, 201);
}
```

Before:
```
public function update(Request $request, $id)
{
    $memory = Memory::findOrFail($id);
    $memory->title = $request->title;
    $memory->description = $request->description;
    $memory->save();

    return redirect
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