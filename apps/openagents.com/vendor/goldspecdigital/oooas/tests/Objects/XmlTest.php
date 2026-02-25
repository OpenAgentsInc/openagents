<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests\Objects;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Xml;
use GoldSpecDigital\ObjectOrientedOAS\Tests\TestCase;

class XmlTest extends TestCase
{
    /** @test */
    public function create_with_all_parameters_works()
    {
        $xml = Xml::create()
            ->name('Xml name')
            ->namespace('xsi:goldspecdigital')
            ->prefix('gsd')
            ->attribute()
            ->wrapped();

        $schema = Schema::object()
            ->xml($xml);

        $this->assertEquals([
            'type' => 'object',
            'xml' => [
                'name' => 'Xml name',
                'namespace' => 'xsi:goldspecdigital',
                'prefix' => 'gsd',
                'attribute' => true,
                'wrapped' => true,
            ],
        ], $schema->toArray());
    }
}
