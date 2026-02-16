<?php

use App\Models\User;
use Illuminate\Support\Facades\Config;

beforeEach(function () {
    Config::set('admin.emails', ['chris@openagents.com']);
});

test('guests are redirected to the login page for admin route', function () {
    $this->get('/admin')->assertRedirect('/login');
});

test('non-admin users are forbidden from admin route', function () {
    $this->actingAs(User::factory()->create([
        'email' => 'not-admin@openagents.com',
    ]));

    $this->get('/admin')->assertForbidden();
});

test('configured admin users can visit admin route', function () {
    $this->actingAs(User::factory()->create([
        'email' => 'CHRIS@openagents.com',
    ]));

    $this->get('/admin')->assertOk();
});
