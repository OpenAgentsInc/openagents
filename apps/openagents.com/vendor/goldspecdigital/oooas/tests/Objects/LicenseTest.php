<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests\Objects;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Info;
use GoldSpecDigital\ObjectOrientedOAS\Objects\License;
use GoldSpecDigital\ObjectOrientedOAS\Tests\TestCase;

class LicenseTest extends TestCase
{
    /** @test */
    public function create_with_all_parameters_works()
    {
        $license = License::create()
            ->name('MIT')
            ->url('https://goldspecdigital.com');

        $info = Info::create()
            ->license($license);

        $this->assertEquals([
            'license' => [
                'name' => 'MIT',
                'url' => 'https://goldspecdigital.com',
            ],
        ], $info->toArray());
    }
}
