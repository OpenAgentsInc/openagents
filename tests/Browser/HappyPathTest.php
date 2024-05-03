<?php

use App\Models\User;
use Laravel\Dusk\Browser;

test('happy path', function () {
    $this->browse(function (Browser $browser) {
        // Homepage loads and shows title and Join button
        $browser
            ->move(150, 50)
            ->resize(1400, 1000)
            ->visit('/')
            ->assertSee('OpenAgents')
            ->assertSee('How can we help you')
            ->assertSee('Join');

        // Mock a login
        $browser->loginAs(User::factory()->create());

        // Can click on Create Agent and go to new create page
        $browser->visit('/')
            ->assertSee('Create an agent')
            ->click('@create-agent')
            ->waitForRoute('create')
            ->assertPathIs('/create')
            ->pause(1500)

            // Can fill out the form to create an agent
            ->type('name', 'Agent Breeder')
            ->pause(150)
            ->type('description', 'Breed ten trillion new AI agents')
            ->pause(150)
            ->type('instructions', 'Your primary objective is to generate ten trillion new AI agents, each with unique characteristics, capabilities, and specializations. To achieve this, you will need to design and implement a robust breeding program that leverages advanced algorithms, machine learning techniques, and innovative problem-solving strategies. The resulting agents should be diverse, adaptable, and capable of operating in a wide range of environments and scenarios. Please begin by developing a comprehensive plan for agent creation, including the definition of agent types, breeding protocols, and evaluation criteria.')
            ->pause(150)
            ->click('@create-agent-button');

        //
    });
});
