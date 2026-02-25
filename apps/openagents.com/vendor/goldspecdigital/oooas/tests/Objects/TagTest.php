<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests\Objects;

use GoldSpecDigital\ObjectOrientedOAS\Objects\ExternalDocs;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Tag;
use GoldSpecDigital\ObjectOrientedOAS\OpenApi;
use GoldSpecDigital\ObjectOrientedOAS\Tests\TestCase;

class TagTest extends TestCase
{
    /** @test */
    public function create_with_all_parameters_works()
    {
        $tag = Tag::create()
            ->name('Users')
            ->description('All user endpoints')
            ->externalDocs(ExternalDocs::create());

        $openApi = OpenApi::create()
            ->tags($tag);

        $this->assertEquals([
            'tags' => [
                [
                    'name' => 'Users',
                    'description' => 'All user endpoints',
                    'externalDocs' => [],
                ],
            ],
        ], $openApi->toArray());
    }
}
