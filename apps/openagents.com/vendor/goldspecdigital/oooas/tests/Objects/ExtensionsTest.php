<?php

namespace GoldSpecDigital\ObjectOrientedOAS\Tests\Objects;

use GoldSpecDigital\ObjectOrientedOAS\Exceptions\PropertyDoesNotExistException;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Components;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Operation;
use GoldSpecDigital\ObjectOrientedOAS\Objects\PathItem;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use GoldSpecDigital\ObjectOrientedOAS\Tests\TestCase;

class ExtensionsTest extends TestCase
{
    /**
     * @test
     * @dataProvider schemasDataProvider
     * @param string|\GoldSpecDigital\ObjectOrientedOAS\Objects\Schema $schema
     */
    public function create_with_extensions($schema)
    {
        $object = $schema::create()
            ->x('key', 'value')
            ->x('x-foo', 'bar')
            ->x('x-baz', null)
            ->x('x-array', Schema::array()->items(Schema::string()));

        $this->assertEquals([
            'x-key' => 'value',
            'x-foo' => 'bar',
            'x-baz' => null,
            'x-array' => Schema::array()->items(Schema::string()),
        ], $object->toArray());

        $this->assertEquals(
            '{"x-key":"value","x-foo":"bar","x-baz":null,"x-array":{"type":"array","items":{"type":"string"}}}',
            $object->toJson()
        );
    }

    /** @test */
    public function can_unset_extensions()
    {
        $object = Schema::create()
            ->x('key', 'value')
            ->x('x-foo', 'bar')
            ->x('x-baz', null);

        $object = $object->x('key');

        $this->assertEquals([
            'x-foo' => 'bar',
            'x-baz' => null,
        ], $object->toArray());

        $this->assertEquals('{"x-foo":"bar","x-baz":null}', $object->toJson());
    }

    /**
     * @test
     * @dataProvider schemasDataProvider
     * @param string|\GoldSpecDigital\ObjectOrientedOAS\Objects\Schema $schema
     */
    public function get_single_extension($schema)
    {
        $object = $schema::create()->x('foo', 'bar');

        $this->assertEquals('bar', $object->{'x-foo'});
    }

    /**
     * @test
     * @dataProvider schemasDataProvider
     * @param string|\GoldSpecDigital\ObjectOrientedOAS\Objects\Schema $schema
     */
    public function get_single_extension_does_not_exist($schema)
    {
        $object = $schema::create()->x('foo', 'bar');

        $this->expectException(PropertyDoesNotExistException::class);
        $this->expectExceptionMessage('[x-key] is not a valid property');
        $this->assertEquals('bar', $object->{'x-key'});
    }

    /**
     * @test
     * @dataProvider schemasDataProvider
     * @param string|\GoldSpecDigital\ObjectOrientedOAS\Objects\Schema $schema
     */
    public function get_all_extensions($schema)
    {
        $object = $schema::create();

        $this->assertEquals([], $object->x);

        $object = $object
            ->x('key', 'value')
            ->x('foo', 'bar');

        $this->assertEquals(['x-key' => 'value', 'x-foo' => 'bar'], $object->x);
    }

    /**
     * @return array
     */
    public function schemasDataProvider(): array
    {
        return [
            [Components::class],
            [Operation::class],
            [PathItem::class],
            [Response::class],
            [Schema::class],
        ];
    }
}
