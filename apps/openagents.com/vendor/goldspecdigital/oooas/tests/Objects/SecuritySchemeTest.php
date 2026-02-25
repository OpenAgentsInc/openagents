<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests\Objects;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Components;
use GoldSpecDigital\ObjectOrientedOAS\Objects\OAuthFlow;
use GoldSpecDigital\ObjectOrientedOAS\Objects\SecurityScheme;
use GoldSpecDigital\ObjectOrientedOAS\Tests\TestCase;

class SecuritySchemeTest extends TestCase
{
    /** @test */
    public function create_with_all_parameters_works()
    {
        $oauthFlow = OAuthFlow::create()
            ->flow(OAuthFlow::FLOW_CLIENT_CREDENTIALS);

        $securityScheme = SecurityScheme::create('OAuth2')
            ->type(SecurityScheme::TYPE_OAUTH2)
            ->description('Standard auth')
            ->in(SecurityScheme::IN_HEADER)
            ->scheme('basic')
            ->bearerFormat('JWT')
            ->flows($oauthFlow)
            ->openIdConnectUrl('https://goldspecdigital.com');

        $components = Components::create()
            ->securitySchemes($securityScheme);

        $this->assertEquals([
            'securitySchemes' => [
                'OAuth2' => [
                    'type' => 'oauth2',
                    'description' => 'Standard auth',
                    'in' => 'header',
                    'scheme' => 'basic',
                    'bearerFormat' => 'JWT',
                    'flows' => [
                        'clientCredentials' => [],
                    ],
                    'openIdConnectUrl' => 'https://goldspecdigital.com',
                ],
            ],
        ], $components->toArray());
    }
}
