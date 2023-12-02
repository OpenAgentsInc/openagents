<?php

namespace Tests\Feature;

use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Foundation\Testing\WithFaker;
use Tests\TestCase;
use App\Models\Memory;

class MemoriesTest extends TestCase
{
    use RefreshDatabase;

    /** @test */
    public function it_can_create_a_memory()
    {
        $memory = Memory::factory()->create();

        $this->post('/api/memories', $memory->toArray())
            ->assertStatus(201)
            ->assertJson($memory->toArray());
    }

    /** @test */
    public function it_can_show_a_memory()
    {
        $memory = Memory::factory()->create();

        $this->get('/api/memories/' . $memory->id)
            ->assertStatus(200)
            ->assertJson($memory->toArray());
    }

    /** @test */
    public function it_can_update_a_memory()
    {
        $memory = Memory::factory()->create();

        $updatedMemory = Memory::factory()->make();

        $this->put('/api/memories/' . $memory->id, $updatedMemory->toArray())
            ->assertStatus(200)
            ->assertJson($updatedMemory->toArray());
    }

    /** @test */
    public function it_can_delete_a_memory()
    {
        $memory = Memory::factory()->create();

        $this->delete('/api/memories/' . $memory->id)
            ->assertStatus(204);
    }
}