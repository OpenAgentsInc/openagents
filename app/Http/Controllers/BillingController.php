<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Illuminate\Routing\Controller;

class BillingController extends Controller
{
    public function stripe_billing_portal(Request $request)
    {
        // User is not a Stripe customer yet. See the createAsStripeCustomer method.
        $user = $request->user();
        if ($user->stripe_id === null) {
            return redirect('/');
        }

        return request()->user()?->redirectToBillingPortal() ?? redirect('/');
    }
}
