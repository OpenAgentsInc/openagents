<?php

/**
 * I’m a developer. I’ve written an agent plugin. Now I want to upload my plugin to OpenAgents and get paid a stream of residual revenue every time my plugin is used in a paid agent flow.
 */

test("user can upload plugin", function () {
    $this->get('/plugins')
        ->assertOk()
        ->assertSee('Upload Plugin');
});
