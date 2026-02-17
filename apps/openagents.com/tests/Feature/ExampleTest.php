<?php

test('home route redirects to chat onboarding', function () {
    $response = $this->get(route('home'));

    $response->assertRedirect('/chat');
});
