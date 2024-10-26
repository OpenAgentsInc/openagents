<?php

namespace App\Http\Controllers;

use Inertia\Inertia;

class InquireController
{
    public function page()
    {
        return Inertia::render('Inquire');
    }
}
