<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Illuminate\Routing\Controller;

class BillingController extends Controller
{
    public function stripe_billing_portal(Request $request)
    {
        return request()->user()?->redirectToBillingPortal() ?? redirect('/');
    }
}
