
<?php

use App\Models\User;

test('authed user cannot trigger a run', function () {
    $user = User::factory()->create();
    $this->actingAs($user);

    $this->post('/faerie-run')->assertForbidden();
});

test('chris can trigger a run', function () {
    $user = User::factory()->create([
        'github_nickname' => 'AtlantisPleb'
    ]);
    $this->actingAs($user);

    $this->post('/faerie-run')->assertOk();
});
