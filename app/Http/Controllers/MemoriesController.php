```php
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
        $memory = Memory::where('id', $id)->firstOrFail();
        return response()->json($memory);
    }

    public function update(Request $request, $id)
    {
        $memory = Memory::where('id', $id)->firstOrFail();
        $memory->fill($request->validated())->save();
        return response()->json($memory);
    }

    public function destroy($id)
    {
        $memory = Memory::where('id', $id)->firstOrFail();
        $memory->delete();
        return response()->json(null, 204);
    }

    public function index()
    {
        return response()->json(Memory::all()->toArray());
    }
}
```