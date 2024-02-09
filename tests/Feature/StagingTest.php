<?php

test("header visible only in non-production environment", function () {
    $this->get("/")
        ->assertSee("Login");

    // Change env var to production
    putenv("APP_ENV=production");

    // dd the env var to see if it's changed
    dd(getenv("APP_ENV"));

    $this->get("/")
        ->assertDontSee("Login");
});
