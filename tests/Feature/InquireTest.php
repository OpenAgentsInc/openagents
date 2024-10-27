<?php

use App\Models\Inquiry;
use Inertia\Testing\AssertableInertia as Assert;

test('inquire page is displayed', function () {
    $response = $this->get('/inquire');

    $response->assertOk();
    $response->assertInertia(fn (Assert $assert) => $assert
        ->component('Inquire')
    );
});

test('inquiry can be submitted', function () {
    $response = $this->post('/inquire', [
        'inquiry_type' => 'custom_agents',
        'email' => 'test@example.com',
        'comment' => 'This is a test inquiry with more than 10 characters',
    ]);

    $response
        ->assertSessionHasNoErrors()
        ->assertRedirect();

    $this->assertDatabaseHas('inquiries', [
        'inquiry_type' => 'custom_agents',
        'email' => 'test@example.com',
        'comment' => 'This is a test inquiry with more than 10 characters',
    ]);
});

test('inquiry requires valid email', function () {
    $response = $this->post('/inquire', [
        'inquiry_type' => 'custom_agents',
        'email' => 'not-an-email',
        'comment' => 'This is a test inquiry',
    ]);

    $response
        ->assertSessionHasErrors('email')
        ->assertRedirect();

    $this->assertDatabaseMissing('inquiries', [
        'email' => 'not-an-email',
    ]);
});

test('inquiry requires comment with minimum length', function () {
    $response = $this->post('/inquire', [
        'inquiry_type' => 'custom_agents',
        'email' => 'test@example.com',
        'comment' => 'too short',
    ]);

    $response
        ->assertSessionHasErrors('comment')
        ->assertRedirect();

    $this->assertDatabaseMissing('inquiries', [
        'email' => 'test@example.com',
        'comment' => 'too short',
    ]);
});

test('inquiry requires both email and comment', function () {
    $response = $this->post('/inquire', [
        'inquiry_type' => 'custom_agents',
        'email' => '',
        'comment' => '',
    ]);

    $response
        ->assertSessionHasErrors(['email', 'comment'])
        ->assertRedirect();

    $this->assertDatabaseCount('inquiries', 0);
});

test('successful inquiry submission shows success message', function () {
    $response = $this->post('/inquire', [
        'inquiry_type' => 'custom_agents',
        'email' => 'test@example.com',
        'comment' => 'This is a valid test inquiry message',
    ]);

    $response
        ->assertSessionHasNoErrors()
        ->assertRedirect()
        ->assertSessionHas('success', 'Thank you for your inquiry. We will get back to you soon.');

    $this->assertDatabaseCount('inquiries', 1);
});

test('inquiry requires valid inquiry type', function () {
    $response = $this->post('/inquire', [
        'inquiry_type' => 'invalid_type',
        'email' => 'test@example.com',
        'comment' => 'This is a test inquiry',
    ]);

    $response
        ->assertSessionHasErrors('inquiry_type')
        ->assertRedirect();

    $this->assertDatabaseCount('inquiries', 0);
});