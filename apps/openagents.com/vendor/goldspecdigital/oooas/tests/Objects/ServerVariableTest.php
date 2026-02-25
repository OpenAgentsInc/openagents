<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests\Objects;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Server;
use GoldSpecDigital\ObjectOrientedOAS\Objects\ServerVariable;
use GoldSpecDigital\ObjectOrientedOAS\Tests\TestCase;

class ServerVariableTest extends TestCase
{
    /** @test */
    public function create_with_all_parameters_works()
    {
        $serverVariable = ServerVariable::create('ServerVariableName')
            ->enum('Earth', 'Mars', 'Saturn')
            ->default('Earth')
            ->description('The planet the server is running on');

        $server = Server::create()
            ->variables($serverVariable);

        $this->assertEquals([
            'variables' => [
                'ServerVariableName' => [
                    'enum' => ['Earth', 'Mars', 'Saturn'],
                    'default' => 'Earth',
                    'description' => 'The planet the server is running on',
                ],
            ],
        ], $server->toArray());
    }
}
