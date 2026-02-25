<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests\Objects;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Example;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Header;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Link;
use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use GoldSpecDigital\ObjectOrientedOAS\Tests\TestCase;

class ResponseTest extends TestCase
{
    /** @test */
    public function create_with_all_parameters_works()
    {
        $header = Header::create('HeaderName')
            ->description('Lorem ipsum')
            ->required()
            ->deprecated()
            ->allowEmptyValue()
            ->style(Header::STYLE_SIMPLE)
            ->explode()
            ->allowReserved()
            ->schema(Schema::string())
            ->example('Example String')
            ->examples(
                Example::create('ExampleName')
                    ->value('Example value')
            )
            ->content(MediaType::json());

        $link = Link::create('MyLink');

        $response = Response::create()
            ->statusCode(200)
            ->description('OK')
            ->headers($header)
            ->content(MediaType::json())
            ->links($link);

        $this->assertEquals(
            [
                'description' => 'OK',
                'headers' => [
                    'HeaderName' => [
                        'description' => 'Lorem ipsum',
                        'required' => true,
                        'deprecated' => true,
                        'allowEmptyValue' => true,
                        'style' => 'simple',
                        'explode' => true,
                        'allowReserved' => true,
                        'schema' => [
                            'type' => 'string',
                        ],
                        'example' => 'Example String',
                        'examples' => [
                            'ExampleName' => [
                                'value' => 'Example value',
                            ],
                        ],
                        'content' => [
                            'application/json' => [],
                        ],
                    ],
                ],
                'content' => [
                    'application/json' => [],
                ],
                'links' => [
                    'MyLink' => [],
                ],
            ],
            $response->toArray()
        );
    }

    /** @test */
    public function create_with_ok_method_works()
    {
        $response = Response::ok();

        $this->assertEquals(200, $response->statusCode);
        $this->assertEquals('OK', $response->description);
    }

    /** @test */
    public function create_with_created_method_works()
    {
        $response = Response::created();

        $this->assertEquals(201, $response->statusCode);
        $this->assertEquals('Created', $response->description);
    }

    /** @test */
    public function create_with_movedPermanently_method_works()
    {
        $response = Response::movedPermanently();

        $this->assertEquals(301, $response->statusCode);
        $this->assertEquals('Moved Permanently', $response->description);
    }

    /** @test */
    public function create_with_movedTemporarily_method_works()
    {
        $response = Response::movedTemporarily();

        $this->assertEquals(302, $response->statusCode);
        $this->assertEquals('Moved Temporarily', $response->description);
    }

    /** @test */
    public function create_with_badRequest_method_works()
    {
        $response = Response::badRequest();

        $this->assertEquals(400, $response->statusCode);
        $this->assertEquals('Bad Request', $response->description);
    }

    /** @test */
    public function create_with_unauthorized_method_works()
    {
        $response = Response::unauthorized();

        $this->assertEquals(401, $response->statusCode);
        $this->assertEquals('Unauthorized', $response->description);
    }

    /** @test */
    public function create_with_notFound_method_works()
    {
        $response = Response::notFound();

        $this->assertEquals(404, $response->statusCode);
        $this->assertEquals('Not Found', $response->description);
    }

    /** @test */
    public function create_with_unprocessableEntity_method_works()
    {
        $response = Response::unprocessableEntity();

        $this->assertEquals(422, $response->statusCode);
        $this->assertEquals('Unprocessable Entity', $response->description);
    }

    /** @test */
    public function create_with_tooManyRequests_method_works()
    {
        $response = Response::tooManyRequests();

        $this->assertEquals(429, $response->statusCode);
        $this->assertEquals('Too Many Requests', $response->description);
    }

    /** @test */
    public function create_with_internalServerError_method_works()
    {
        $response = Response::internalServerError();

        $this->assertEquals(500, $response->statusCode);
        $this->assertEquals('Internal Server Error', $response->description);
    }
}
