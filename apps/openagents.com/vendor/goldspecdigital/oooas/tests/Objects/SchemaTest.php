<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests\Objects;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Discriminator;
use GoldSpecDigital\ObjectOrientedOAS\Objects\ExternalDocs;
use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\OneOf;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Xml;
use GoldSpecDigital\ObjectOrientedOAS\Tests\TestCase;

class SchemaTest extends TestCase
{
    /** @test */
    public function create_array_with_all_parameters_works()
    {
        $schema = Schema::create()
            ->title('Schema title')
            ->description('Schema description')
            ->enum(['Earth'], ['Venus'], ['Mars'])
            ->default(['Earth'])
            ->type(Schema::TYPE_ARRAY)
            ->items(Schema::string())
            ->maxItems(10)
            ->minItems(1)
            ->uniqueItems()
            ->nullable()
            ->discriminator(Discriminator::create()->propertyName('Property name'))
            ->readOnly()
            ->writeOnly()
            ->xml(Xml::create())
            ->externalDocs(ExternalDocs::create()->url('https://goldspecdigital.com'))
            ->example(['Venus'])
            ->deprecated();

        $response = MediaType::create()
            ->schema($schema);

        $this->assertEquals([
            'schema' => [
                'title' => 'Schema title',
                'description' => 'Schema description',
                'enum' => [
                    ['Earth'],
                    ['Venus'],
                    ['Mars'],
                ],
                'default' => ['Earth'],
                'type' => 'array',
                'items' => ['type' => 'string'],
                'maxItems' => 10,
                'minItems' => 1,
                'uniqueItems' => true,
                'nullable' => true,
                'discriminator' => ['propertyName' => 'Property name'],
                'readOnly' => true,
                'writeOnly' => true,
                'xml' => [],
                'externalDocs' => ['url' => 'https://goldspecdigital.com'],
                'example' => ['Venus'],
                'deprecated' => true,
            ],
        ], $response->toArray());
    }

    /** @test */
    public function create_boolean_with_all_parameters_works()
    {
        $schema = Schema::create()
            ->title('Schema title')
            ->description('Schema description')
            ->default(false)
            ->type(Schema::TYPE_BOOLEAN)
            ->nullable()
            ->discriminator(Discriminator::create()->propertyName('Property name'))
            ->readOnly()
            ->writeOnly()
            ->xml(Xml::create())
            ->externalDocs(ExternalDocs::create()->url('https://goldspecdigital.com'))
            ->example(['Venus'])
            ->deprecated();

        $response = MediaType::create()
            ->schema($schema);

        $this->assertEquals([
            'schema' => [
                'title' => 'Schema title',
                'description' => 'Schema description',
                'default' => false,
                'type' => 'boolean',
                'nullable' => true,
                'discriminator' => ['propertyName' => 'Property name'],
                'readOnly' => true,
                'writeOnly' => true,
                'xml' => [],
                'externalDocs' => ['url' => 'https://goldspecdigital.com'],
                'example' => ['Venus'],
                'deprecated' => true,
            ],
        ], $response->toArray());
    }

    /** @test */
    public function create_integer_with_all_parameters_works()
    {
        $schema = Schema::create()
            ->title('Schema title')
            ->description('Schema description')
            ->default(false)
            ->format(Schema::FORMAT_INT32)
            ->type(Schema::TYPE_INTEGER)
            ->maximum(100)
            ->exclusiveMaximum(101)
            ->minimum(1)
            ->exclusiveMinimum(0)
            ->multipleOf(2)
            ->nullable()
            ->discriminator(Discriminator::create()->propertyName('Property name'))
            ->readOnly()
            ->writeOnly()
            ->xml(Xml::create())
            ->externalDocs(ExternalDocs::create()->url('https://goldspecdigital.com'))
            ->example(['Venus'])
            ->deprecated();

        $response = MediaType::create()
            ->schema($schema);

        $this->assertEquals([
            'schema' => [
                'title' => 'Schema title',
                'description' => 'Schema description',
                'default' => false,
                'format' => 'int32',
                'type' => 'integer',
                'maximum' => 100,
                'exclusiveMaximum' => 101,
                'minimum' => 1,
                'exclusiveMinimum' => 0,
                'multipleOf' => 2,
                'nullable' => true,
                'discriminator' => ['propertyName' => 'Property name'],
                'readOnly' => true,
                'writeOnly' => true,
                'xml' => [],
                'externalDocs' => ['url' => 'https://goldspecdigital.com'],
                'example' => ['Venus'],
                'deprecated' => true,
            ],
        ], $response->toArray());
    }

    /** @test */
    public function create_number_with_all_parameters_works()
    {
        $schema = Schema::create()
            ->title('Schema title')
            ->description('Schema description')
            ->default(false)
            ->type(Schema::TYPE_NUMBER)
            ->maximum(100)
            ->exclusiveMaximum(101)
            ->minimum(1)
            ->exclusiveMinimum(0)
            ->multipleOf(2)
            ->nullable()
            ->discriminator(Discriminator::create()->propertyName('Property name'))
            ->readOnly()
            ->writeOnly()
            ->xml(Xml::create())
            ->externalDocs(ExternalDocs::create()->url('https://goldspecdigital.com'))
            ->example(['Venus'])
            ->deprecated();

        $response = MediaType::create()
            ->schema($schema);

        $this->assertEquals([
            'schema' => [
                'title' => 'Schema title',
                'description' => 'Schema description',
                'default' => false,
                'type' => 'number',
                'maximum' => 100,
                'exclusiveMaximum' => 101,
                'minimum' => 1,
                'exclusiveMinimum' => 0,
                'multipleOf' => 2,
                'nullable' => true,
                'discriminator' => ['propertyName' => 'Property name'],
                'readOnly' => true,
                'writeOnly' => true,
                'xml' => [],
                'externalDocs' => ['url' => 'https://goldspecdigital.com'],
                'example' => ['Venus'],
                'deprecated' => true,
            ],
        ], $response->toArray());
    }

    /** @test */
    public function create_object_with_all_parameters_works()
    {
        $property = Schema::string('id')
            ->format(Schema::FORMAT_UUID);

        $schema = Schema::create()
            ->title('Schema title')
            ->description('Schema description')
            ->default(false)
            ->type(Schema::TYPE_OBJECT)
            ->required($property)
            ->properties($property)
            ->additionalProperties(Schema::integer('age'))
            ->maxProperties(10)
            ->minProperties(1)
            ->nullable()
            ->discriminator(Discriminator::create()->propertyName('Property name'))
            ->readOnly()
            ->writeOnly()
            ->xml(Xml::create())
            ->externalDocs(ExternalDocs::create()->url('https://goldspecdigital.com'))
            ->example(['Venus'])
            ->deprecated();

        $response = MediaType::create()
            ->schema($schema);

        $this->assertEquals([
            'schema' => [
                'title' => 'Schema title',
                'description' => 'Schema description',
                'default' => false,
                'type' => 'object',
                'required' => ['id'],
                'properties' => [
                    'id' => [
                        'type' => 'string',
                        'format' => 'uuid',
                    ],
                ],
                'additionalProperties' => [
                    'type' => 'integer',
                ],
                'maxProperties' => 10,
                'minProperties' => 1,
                'nullable' => true,
                'discriminator' => ['propertyName' => 'Property name'],
                'readOnly' => true,
                'writeOnly' => true,
                'xml' => [],
                'externalDocs' => ['url' => 'https://goldspecdigital.com'],
                'example' => ['Venus'],
                'deprecated' => true,
            ],
        ], $response->toArray());
    }

    /** @test */
    public function create_string_with_all_parameters_works()
    {
        $schema = Schema::create()
            ->title('Schema title')
            ->description('Schema description')
            ->default(false)
            ->format(Schema::FORMAT_UUID)
            ->type(Schema::TYPE_STRING)
            ->pattern('/[a-zA-Z]+/')
            ->maxLength(10)
            ->minLength(1)
            ->nullable()
            ->discriminator(Discriminator::create()->propertyName('Property name'))
            ->readOnly()
            ->writeOnly()
            ->xml(Xml::create())
            ->externalDocs(ExternalDocs::create()->url('https://goldspecdigital.com'))
            ->example(['Venus'])
            ->deprecated();

        $response = MediaType::create()
            ->schema($schema);

        $this->assertEquals([
            'schema' => [
                'title' => 'Schema title',
                'description' => 'Schema description',
                'default' => false,
                'format' => 'uuid',
                'type' => 'string',
                'pattern' => '/[a-zA-Z]+/',
                'maxLength' => 10,
                'minLength' => 1,
                'nullable' => true,
                'discriminator' => ['propertyName' => 'Property name'],
                'readOnly' => true,
                'writeOnly' => true,
                'xml' => [],
                'externalDocs' => ['url' => 'https://goldspecdigital.com'],
                'example' => ['Venus'],
                'deprecated' => true,
            ],
        ], $response->toArray());
    }

    /** @test */
    public function create_array_with_ref_works()
    {
        $schema = Schema::array()
            ->items(
                Schema::ref('#/components/schemas/pet')
            );

        $this->assertEquals([
            'type' => 'array',
            'items' => [
                '$ref' => '#/components/schemas/pet',
            ],
        ], $schema->toArray());
    }

    /** @test */
    public function create_object_with_oneOf_works()
    {
        $string = Schema::string();
        $number = Schema::number();

        $schema = Schema::create()
            ->type(Schema::TYPE_OBJECT)
            ->properties(
                OneOf::create('poly_type')->schemas($string, $number)
            );

        $response = MediaType::create()
            ->schema($schema);

        $this->assertEquals([
            'schema' => [
                'type' => 'object',
                'properties' => [
                    'poly_type' => [
                        'oneOf' => [
                            [
                                'type' => 'string',
                            ],
                            [
                                'type' => 'number',
                            ],
                        ],
                    ],
                ],
            ],
        ], $response->toArray());
    }
}
