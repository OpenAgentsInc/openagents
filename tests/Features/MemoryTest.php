<?php

use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class MemoryTest extends TestCase
{
    use RefreshDatabase;

    public function test_can_store_memory()
    {
        $memory = [
            'title' => 'Test Memory',
            'description' => 'This is a test memory',
            'date' => '2021-01-01',
            'location' => 'Test Location',
            'image' => 'test_image.jpg',
        ];

        $this->post(route('memories.store'), $memory)
            ->assertStatus(201)
            ->assertJson($memory);
    }

    public function test_can_get_all_memories()
    {
        $memories = factory(Memory::class, 5)->create();

        $this->get(route('memories.index'))
            ->assertStatus(200)
            ->assertJson($memories->toArray());
    }

    public function test_can_get_memory()
    {
        $memory = factory(Memory::class)->create();

        $this->get(route('memories.show', $memory->id))
            ->assertStatus(200)
            ->assertJson($memory->toArray());
    }

    public function test_can_update_memory()
    {
        $memory = factory(Memory::class)->create();

        $updatedMemory = [
            'title' => 'Updated Memory',
            'description' => 'This is an updated memory',
            'date' => '2021-01-02',
            'location' => 'Updated Location',
            'image' => 'updated_image.jpg',
        ];

        $this->put(route('memories.update', $memory->id), $updatedMemory)
            ->assertStatus(200)
            ->assertJson($updatedMemory);
    }

    public function test_can_delete_memory()
    {
        $memory = factory(Memory::class)->create();

        $this->delete(route('memories.destroy', $memory->id))
            ->assertStatus(204);
    }
}