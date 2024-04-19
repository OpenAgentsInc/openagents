<?php

namespace Tests;

use Illuminate\Foundation\Testing\TestCase as BaseTestCase;

abstract class TestCase extends BaseTestCase
{
    // define skipInProduction method
    public function skipInProduction()
    {
        dump('env is '.app()->environment());

        if (app()->environment('production')) {
            $this->markTestSkipped('Skipping this test in production.');
        }
    }
}
