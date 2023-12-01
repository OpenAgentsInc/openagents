<?php

namespace Tests\Feature;

use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Foundation\Testing\WithFaker;
use Tests\TestCase;
use App\Models\Memory;

class MemoriesTest extends TestCase
{
    use RefreshDatabase;

    public function test_create_memory()
    {
$memory = factory(Memory::class)->make();

$response = $this->post('/memories', $memory->toArray());

$response->assertStatus(201);
$this->assertDatabaseHas('memories', ['title' => $memory->title]);
```

Before:
```
$memory = Memory::factory()->create();

$response = $this->get('/memories/' . $memory->id);

$response->assertStatus(200);
$response->assertJson($memory->toArray());
```

After:
```
$memory = factory(Memory::class)->create();

$response = $this->get('/memories/' . $memory->id);

$response->assertStatus(200);
$response->assertJson(['title' => $memory->title]);
```

Before:
```
$memory = Memory::factory()->create();
$newMemory = Memory::factory()->make();

$response = $this->put('/memories/' . $memory->id, $newMemory->toArray());

$response->assertStatus(200);
$this->assertDatabaseHas('memories', $newMemory->toArray());
$this->assertDatabaseMissing('memories', $memory->toArray());
```

After:
```
$memory = factory(Memory::class)->create();
$newMemory = factory(Memory::class)->make();

$response = $this->put('/memories/' . $memory->id, $newMemory->toArray());

$response->assertStatus(200);
$this->assertDatabaseHas('memories', ['title' => $newMemory->title]);
$this->assertDatabaseMissing('memories', ['title' => $memory->title]);
```

Before:
```
$memory = Memory::factory()->create();

$response = $this->delete('/memories/' . $memory->id);

$response->assertStatus(200);
$this->assertDatabaseMissing('memories', $memory->toArray());
```

After:
```
$memory = factory(Memory::class)->create();

$response = $this->delete('/memories/' . $memory->id);

$response->assertStatus(200);
$this->assertDatabaseMissing('memories', ['title' => $memory->title]);
```

Before:
```
public function test_create_memory()
{
    $memory = Memory::factory()->make();

    $response = $this->post('/memories', $memory->toArray());

    $response->assertStatus(201);
    $this->assertDatabaseHas('memories', $memory->toArray());
}
```

After:
```
public function test_create_memory()
{
    $memory = factory(Memory::class)->make();

    $response = $this->post('/memories', $memory->toArray());

    $response->assertStatus(201);
    $this->assertDatabaseHas('memories', ['title' => $memory->title]);
}
```

Before:
```
public function test_update_memory()
{
    $memory = Memory::factory()->create();
    $newMemory = Memory::factory()->make();

    $response = $this->put('/memories/' . $memory->id, $newMemory->toArray());

    $response->assertStatus(200);
    $this->assertDatabaseHas('memories', $newMemory->toArray());
    $this->assertDatabaseMissing('memories', $memory->toArray());
}
```

After:
```
public function test_update_memory()
{
    $memory = factory(Memory::class)->create();
    $newMemory = factory(Memory::class)->make();

    $response = $this->put('/memories/' . $memory->id, $newMemory->toArray());

    $response->assertStatus(200);
    $this->assertDatabaseHas('memories', ['title' => $newMemory->title]);
    $this->assertDatabaseMissing('memories', ['title' => $memory->title]);
}
```

Before:
```
public function test_delete_memory()
{
    $memory = Memory::factory()->create();

    $response = $this->delete('/memories/' . $memory->id);

    $response->assertStatus(200);
    $this->assertDatabaseMissing('memories', $memory->toArray());
}
```

After:
```
public function test_delete_memory()
{
    $memory = factory(Memory::class)->create();

    $response = $this->delete('/memories/' . $memory->id);

    $response->assertStatus(200);
    $this->assertDatabaseMissing('memories', ['title' => $memory->title]);
}
```

Before:
```
public function test_show_memory()
{
    $memory = Memory::factory()->create();

    $response = $this->get('/memories/' . $memory->
}

    public function test_show_memory()
    {
        $memory = Memory::factory()->create();

        $response = $this->get('/memories/' . $memory->id);

        $response->assertStatus(200);
        $response->assertJson($memory->toArray());
    }

    public function test_update_memory()
    {
        $memory = Memory::factory()->create();
        $newMemory = Memory::factory()->make();

        $response = $this->put('/memories/' . $memory->id, $newMemory->toArray());

        $response->assertStatus(200);
        $this->assertDatabaseHas('memories', $newMemory->toArray());
        $this->assertDatabaseMissing('memories', $memory->toArray());
    }

    public function test_delete_memory()
    {
        $memory = Memory::factory()->create();

        $response = $this->delete('/memories/' . $memory->id);

        $response->assertStatus(200);
        $this->assertDatabaseMissing('memories', $memory->toArray());
    }
}