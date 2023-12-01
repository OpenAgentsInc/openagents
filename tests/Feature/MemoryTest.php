<?php

namespace Tests\Feature;

use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Foundation\Testing\WithFaker;
use Tests\TestCase;
use App\Models\Memory;

class MemoryTest extends TestCase
{
    use RefreshDatabase;

    public function testStore()
    {
        $memory = Memory::factory()->create();

        $response = $this->post('/api/memories', $memory->toArray());

        $response->assertStatus(201);
        $this->assertDatabaseHas('memories', $memory->toArray());
    }

    public function testShow()
    {
        $memory = Memory::factory()->create();

        $response = $this->get('/api/memories/' . $memory->id);

        $response->assertStatus(200);
        $response->assertJson($memory->toArray());
    }

    public function testUpdate()
    {
        $memory = Memory::factory()->create();

        $updatedMemory = Memory::factory()->make();

        $response = $this->put('/api/memories/' . $memory->id, $updatedMemory->toArray());

        $response->assertStatus(200);
        $this->assertDatabaseHas('memories', $updatedMemory->toArray());
    }

    public function testDestroy()
    {
        $memory = Memory::factory()->create();

        $response = $this->delete('/api/memories/' . $memory->id);

        $response->assertStatus(204);
        $this->assertDatabaseMissing('memories', $memory->toArray());
    }
}