<?php

test('user can see create agent form', function () {
    $this->get('/agents/create')
        ->assertOk()
        ->assertSee('Create Agent')
        ->assertViewIs('agent-create');
});
