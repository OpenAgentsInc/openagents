<?php

test("header visible only in non-production environment", function () {
    // $this->get("/")
    //     ->assertSee("Login");

    config(["app.env" => "production"]);

    // Clear artisan config cache
    $this->artisan("config:clear");
    $this->artisan("view:clear");

    $this->get("/")
        ->assertDontSee("Login");
});
