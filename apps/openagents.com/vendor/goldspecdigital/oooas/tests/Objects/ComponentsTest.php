<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests\Objects;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Components;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Example;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Header;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Link;
use GoldSpecDigital\ObjectOrientedOAS\Objects\OAuthFlow;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Operation;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Parameter;
use GoldSpecDigital\ObjectOrientedOAS\Objects\PathItem;
use GoldSpecDigital\ObjectOrientedOAS\Objects\RequestBody;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use GoldSpecDigital\ObjectOrientedOAS\Objects\SecurityScheme;
use GoldSpecDigital\ObjectOrientedOAS\Tests\TestCase;

class ComponentsTest extends TestCase
{
    /** @test */
    public function create_with_all_parameters_works()
    {
        $schema = Schema::object('ExampleSchema');

        $response = Response::created('ResourceCreated');

        $parameter = Parameter::query('Page')
            ->name('page');

        $example = Example::create('PageExample')
            ->value(5);

        $requestBody = RequestBody::create('CreateResource');

        $header = Header::create('HeaderExample');

        $oauthFlow = OAuthFlow::create()
            ->flow(OAuthFlow::FLOW_IMPLICIT)
            ->authorizationUrl('http://example.org/api/oauth/dialog');

        $securityScheme = SecurityScheme::create('OAuth2')
            ->type(SecurityScheme::TYPE_OAUTH2)
            ->flows($oauthFlow);

        $link = Link::create('LinkExample');

        $callback = PathItem::create('MyEvent')
            ->route('{$request.query.callbackUrl}')
            ->operations(
                Operation::post()->requestBody(
                    RequestBody::create()
                        ->description('something happened')
                )
            );

        $components = Components::create()
            ->schemas($schema)
            ->responses($response)
            ->parameters($parameter)
            ->examples($example)
            ->requestBodies($requestBody)
            ->headers($header)
            ->securitySchemes($securityScheme)
            ->links($link)
            ->callbacks($callback);

        $this->assertEquals([
            'schemas' => [
                'ExampleSchema' => [
                    'type' => 'object',
                ],
            ],
            'responses' => [
                'ResourceCreated' => [
                    'description' => 'Created',
                ],
            ],
            'parameters' => [
                'Page' => [
                    'name' => 'page',
                    'in' => 'query',
                ],
            ],
            'examples' => [
                'PageExample' => [
                    'value' => 5,
                ],
            ],
            'requestBodies' => [
                'CreateResource' => [],
            ],
            'headers' => [
                'HeaderExample' => [],
            ],
            'securitySchemes' => [
                'OAuth2' => [
                    'type' => 'oauth2',
                    'flows' => [
                        'implicit' => [
                            'authorizationUrl' => 'http://example.org/api/oauth/dialog',
                        ],
                    ],
                ],
            ],
            'links' => [
                'LinkExample' => [],
            ],
            'callbacks' => [
                'MyEvent' => [
                    '{$request.query.callbackUrl}' => [
                        'post' => [
                            'requestBody' => [
                                'description' => 'something happened',
                            ],
                        ],
                    ],
                ],
            ],
        ], $components->toArray());
    }
}
