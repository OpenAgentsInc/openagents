<?php

namespace App\Http\Controllers;

use Inertia\Inertia;

class CRMController
{
    public function index()
    {
        return Inertia::render('CRM');
    }
}
