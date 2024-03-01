<?php

use App\Models\Node;
use App\Models\User;

test('can create node via api', function () {
    $this->actingAs(User::factory()->create());

    $response = $this->postJson('/api/v1/nodes', [
        'name' => 'URL Extractor',
        'type' => 'plugin',
    ]);

    $response->assertStatus(200);

    expect(Node::count())->toBe(1);

    $this->assertDatabaseHas('nodes', [
        'name' => 'URL Extractor',
        'type' => 'plugin',
    ]);
});
