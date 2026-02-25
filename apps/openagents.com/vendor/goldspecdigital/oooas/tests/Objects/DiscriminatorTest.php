<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests\Objects;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Discriminator;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use GoldSpecDigital\ObjectOrientedOAS\Tests\TestCase;

class DiscriminatorTest extends TestCase
{
    /** @test */
    public function create_with_all_parameters_works()
    {
        $discriminator = Discriminator::create()
            ->propertyName('Discriminator Name')
            ->mapping(['key' => 'value']);

        $schema = Schema::object()
            ->discriminator($discriminator);

        $this->assertEquals([
            'type' => 'object',
            'discriminator' => [
                'propertyName' => 'Discriminator Name',
                'mapping' => [
                    'key' => 'value',
                ],
            ],
        ], $schema->toArray());
    }
}
