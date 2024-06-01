<?php

it('renders a list of plugins', function () {
    $this->get('/plugins')
        ->assertStatus(200)
        ->assertSee('Plugin Registry');
    // ->assertSee('RSS Feed Reader Plugin')
    // ->assertSee('US Zip Code Information Plugin')
    // ->assertSee('Key-Value storage Plugin')
    // ->assertSee('Fact Checker Plugin');
});
