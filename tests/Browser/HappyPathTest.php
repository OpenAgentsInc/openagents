<?php

use Laravel\Dusk\Browser;

test('Happy Path', function () {
    $this->browse(function (Browser $browser) {
        $browser->visit('/')

            // I see OpenAgents
            ->waitForText('OpenAgents')
            ->assertSee('OpenAgents')

            // I see the how can we help section with form
            ->assertSee('How can we help?')

            // I can submit the form
            ->type('content', 'What does this do')
            ->click('button[type="submit"]')

            // The URL updates to chat/{id}
            ->waitForLocation('/chat/1')

            // I see the message I sent on this page
            ->assertSee('What does this do')

            // And I see a loading indicator that my message is being responded to
            ->assertSee('Typing...')

            // I see login buttons
            ->assertSee('Log in')
            ->assertSee('Sign up')

            // I can click sign up
            ->clickLink('Sign up')

            // The signup page loads
            ->waitForText('Sign up for OpenAgents')
            ->assertPathIs('/register')

            // I see a form and can fill it out (name, email, password, confirm)
            ->type('name', 'John Doe')
            ->type('email', 'joe@jammersmith.com')
            ->type('password', 'password')
            ->type('password_confirmation', 'password')

            // I can submit the form
            ->click('button[type="submit"]')

            // I see the form with prompt
            ->waitForText('How can we help?')
            ->assertSee('How can we help?')

            ->screenshot('done');
    });
});
