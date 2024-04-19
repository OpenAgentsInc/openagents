<?php

namespace Tests;

use Illuminate\Foundation\Testing\TestCase as BaseTestCase;

abstract class TestCase extends BaseTestCase
{
    public function skipInCI()
    {
        if (app()->environment('testing')) {
            $this->markTestSkipped('Skipping this test in production.');
        }
    }
}
