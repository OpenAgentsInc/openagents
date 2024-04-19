<?php

namespace Tests;

use Illuminate\Foundation\Testing\TestCase as BaseTestCase;

abstract class TestCase extends BaseTestCase
{
    // define skipInProduction method
    public function skipInProduction()
    {
        if (app()->environment('production')) {
            $this->markTestSkipped('Skipping this test in production.');
        }
    }
}
