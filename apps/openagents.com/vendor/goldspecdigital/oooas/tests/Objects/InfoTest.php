<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests\Objects;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Contact;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Info;
use GoldSpecDigital\ObjectOrientedOAS\Objects\License;
use GoldSpecDigital\ObjectOrientedOAS\OpenApi;
use GoldSpecDigital\ObjectOrientedOAS\Tests\TestCase;

class InfoTest extends TestCase
{
    /** @test */
    public function create_with_all_parameters_works()
    {
        $info = Info::create()
            ->title('Pretend API')
            ->description('A pretend API')
            ->termsOfService('https://goldspecdigital.com')
            ->contact(Contact::create())
            ->license(License::create())
            ->version('v1');

        $openApi = OpenApi::create()
            ->info($info);

        $this->assertEquals([
            'info' => [
                'title' => 'Pretend API',
                'description' => 'A pretend API',
                'termsOfService' => 'https://goldspecdigital.com',
                'contact' => [],
                'license' => [],
                'version' => 'v1',
            ],
        ], $openApi->toArray());
    }
}
