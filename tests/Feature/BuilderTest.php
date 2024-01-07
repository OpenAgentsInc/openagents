<?php

test('authed user can visit builder page', function () {
    $this->actingAs(\App\Models\User::factory()->create())
        ->get('/builder')
        ->assertStatus(200);
});
