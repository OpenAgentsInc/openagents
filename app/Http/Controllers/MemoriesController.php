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
```

Before:
```
use PHPUnit\Framework\TestCase;

class MemoriesTest extends TestCase
{
    public function test_can_create_memory()
    {
        $memory = Memory::factory()->create();
        $this->assertDatabaseHas('memories', $memory->toArray());
    }

    public function test_can_get_memory()
    {
        $memory = Memory::factory()->create();
        $response = $this->get('/memories/' . $memory->id);
        $response->assertStatus(200);
    }

    public function test_can_update_memory()
    {
        $memory = Memory::factory()->create();
        $new_data = ['title' => 'New Title'];
        $response = $this->put('/memories/' . $memory->id, $new_data);
        $response->assertStatus(200);
        $this->assertEquals($new_data['title'], $memory->fresh()->title);
    }

    public function test_can_delete_memory()
    {
        $memory = Memory::factory()->create();
        $response = $this->delete('/memories/' . $memory->id);
        $response->assertStatus(204);
        $this->assertDatabaseMissing('memories', $memory->toArray());
    }
}
```

After:
```
use Tests\TestCase;
use App\Models\Memory;
use App\Database\Factories\MemoryFactory;

it('can create memory', function () {
    $memory = MemoryFactory::new()->create();
    expect(Memory::find($memory->id))->toBe($memory);
});

it('can get memory', function () {
    $memory = MemoryFactory::new()->create();
    $response = $this->get('/memories/' . $memory->id);
    $response->assertStatus(200);
});

it('can update memory', function () {
    $memory = MemoryFactory::new()->create();
    $new_data = ['title' => 'New Title'];
    $response = $this->put('/memories/' . $memory->id, $new_data);
    $response->assertStatus(200);
    expect($memory->fresh()->title)->toBe($new_data['title']);
});

it('can delete memory', function () {
    $memory = MemoryFactory::new()->create();
    $response = $this->delete('/memories/' . $memory->id);
    $response->assertStatus(204);
    expect(Memory::find($memory->id))->toBeNull();
});
}