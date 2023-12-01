<?php

namespace Tests\Feature;

use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Foundation\Testing\WithFaker;
use Tests\TestCase;
use App\Models\Memory;

class MemoryTest extends TestCase
{
    use RefreshDatabase;

    /** @test */
    public function it_can_create_a_memory()
    {
        $memory = Memory::factory()->make();

        $this->post('/memories', $memory->toArray());

        $this->assertDatabaseHas('memories', $memory->toArray());
    }

    /** @test */
    public function it_can_read_a_memory()
    {
        $memory = Memory::factory()->create();

        $this->get('/memories/' . $memory->id);

        $this->assertEquals($memory->toArray(), $this->response->getContent());
    }

    /** @test */
    public function it_can_update_a_memory()
    {
        $memory = Memory::factory()->create();

        $updatedMemory = Memory::factory()->make();

        $this->put('/memories/' . $memory->id, $updatedMemory->toArray());

        $this->assertDatabaseHas('memories', $updatedMemory->toArray());
    }

    /** @test */
    public function it_can_delete_a_memory()
    {
        $memory = Memory::factory()->create();

        $this->delete('/memories/' . $memory->id);

        $this->assertDatabaseMissing('memories', $memory->toArray());
    }
}