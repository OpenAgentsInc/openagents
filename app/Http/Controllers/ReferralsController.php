<?php

namespace App\Http\Controllers;

use Inertia\Inertia;

class ReferralsController extends Controller
{
    public function referrals()
    {
        return Inertia::render('Referrals', [
            'referrals' => auth()->user()->referrals()->get(),
        ]);
    }
}
