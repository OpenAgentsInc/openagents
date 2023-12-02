New Content:

```php
<?php

namespace Tests\Feature;

use App\Models\Memory;
use Illuminate\Foundation\Testing\RefreshDatabase;

class MemoriesControllerTest extends TestCase
{
    use RefreshDatabase;

    public function test_can_create_a_memory_from_a_POST_request()
    {
        $memory = Memory::factory()->make();
        $response = $this->post('/memories', $memory->toArray());
        $response->assertStatus(201);
        $this->assertDatabaseHas('memories', $memory->toArray());
    }

    public function test_can_display_a_memory_from_a_GET_request()
    {
        $memory = Memory::factory()->create();
        $response = $this->get('/memories/' . $memory->id);
        $response->assertStatus(200);
        $response->assertJson($memory->toArray());
    }

    public function test_can_update_a_memory_from_a_PUT_request()
    {
        $memory = Memory::factory()->create();
        $newMemory = Memory::factory()->make();
        $response = $this->put('/memories/' . $memory->id, $newMemory->toArray());
        $response->assertStatus(200);
        $this->assertDatabaseHas('memories', $newMemory->toArray());
    }

    public function test_can_delete_a_memory_from_a_DELETE_request()
    {
        $memory = Memory::factory()->create();
        $response = $this->delete('/memories/' . $memory->id);
        $response->assertStatus(204);
        $this->assertDatabaseMissing('memories', $memory->toArray());
    }
}
```
