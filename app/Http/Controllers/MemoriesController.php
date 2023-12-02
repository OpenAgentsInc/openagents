The original content doesn't seem to have any syntax errors or logical flaws. Thus, the provided cleaned code block would be the same as the original one:

```php
<?php

namespace App\Http\Controllers;

use App\Models\Memory;
use Illuminate\Http\Request;

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
```
