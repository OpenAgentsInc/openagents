<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests\Objects;

use GoldSpecDigital\ObjectOrientedOAS\Objects\OAuthFlow;
use GoldSpecDigital\ObjectOrientedOAS\Objects\SecurityScheme;
use GoldSpecDigital\ObjectOrientedOAS\Tests\TestCase;

class OAuthFlowTest extends TestCase
{
    /** @test */
    public function create_with_all_parameters_works()
    {
        $oauthFlow = OAuthFlow::create()
            ->flow(OAuthFlow::FLOW_AUTHORIZATION_CODE)
            ->authorizationUrl('https://api.goldspecdigital.com/oauth/authorization')
            ->tokenUrl('https://api.goldspecdigital.com/oauth/token')
            ->refreshUrl('https://api.goldspecdigital.com/oauth/token')
            ->scopes(['read:user' => 'Read the user profile']);

        $securityScheme = SecurityScheme::create()
            ->flows($oauthFlow);

        $this->assertEquals([
            'flows' => [
                'authorizationCode' => [
                    'authorizationUrl' => 'https://api.goldspecdigital.com/oauth/authorization',
                    'tokenUrl' => 'https://api.goldspecdigital.com/oauth/token',
                    'refreshUrl' => 'https://api.goldspecdigital.com/oauth/token',
                    'scopes' => [
                        'read:user' => 'Read the user profile',
                    ],
                ],
            ],
        ], $securityScheme->toArray());
    }
}
