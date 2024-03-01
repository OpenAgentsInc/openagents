<?php

use App\Models\User;

test('can create node via api', function () {
    $this->actingAs(User::factory()->create());

    $response = $this->postJson('/api/v1/nodes', [
        'name' => 'URL Extractor',
        'type' => 'plugin',
    ]);

    $response->assertStatus(200);
});
