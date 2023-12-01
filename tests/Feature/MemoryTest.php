<?php

namespace Tests\Feature;

use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Foundation\Testing\WithFaker;
use Tests\TestCase;

class MemoryTest extends TestCase
{
    use RefreshDatabase;

    public function test_store_method()
    {
        $response = $this->post('/api/memories', [
            'title' => 'Test Memory',
            'description' => 'This is a test memory',
            'date' => '2021-01-01',
            'location' => 'Test Location'
        ]);

        $response->assertStatus(201);
        $this->assertDatabaseHas('memories', [
            'title' => 'Test Memory',
            'description' => 'This is a test memory',
            'date' => '2021-01-01',
            'location' => 'Test Location'
        ]);
    }

    public function test_show_method()
    {
        $memory = factory(\App\Memory::class)->create();

        $response = $this->get('/api/memories/' . $memory->id);

        $response->assertStatus(200);
        $response->assertJson([
            'title' => $memory->title,
            'description' => $memory->description,
            'date' => $memory->date,
            'location' => $memory->location
        ]);
    }

    public function test_update_method()
    {
        $memory = factory(\App\Memory::class)->create();

        $response = $this->put('/api/memories/' . $memory->id, [
            'title' => 'Updated Memory',
            'description' => 'This is an updated memory',
            'date' => '2021-02-01',
            'location' => 'Updated Location'
        ]);

        $response->assertStatus(200);
        $this->assertDatabaseHas('memories', [
            'id' => $memory->id,
            'title' => 'Updated Memory',
            'description' => 'This is an updated memory',
            'date' => '2021-02-01',
            'location' => 'Updated Location'
        ]);
    }

    public function test_destroy_method()
    {
        $memory = factory(\App\Memory::class)->create();

        $response = $this->delete('/api/memories/' . $memory->id);

        $response->assertStatus(204);
        $this->assertDatabaseMissing('memories', [
            'id' => $memory->id
        ]);
    }
}