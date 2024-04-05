<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Illuminate\Routing\Controller;

class BillingController extends Controller
{
    public function stripe_subscribe(Request $request)
    {
        $user = $request->user();
        if (! $user) {
            return redirect('/');
        }

        return $user
            ->newSubscription('default', 'price_1P2LIcBBavRcGMZrw4YTGSIT')
            ->checkout([
                'success_url' => route('home'),
                'cancel_url' => route('home'),
            ]);

    }

    public function stripe_billing_portal(Request $request)
    {
        $user = $request->user();
        if (! $user) {
            return redirect('/');
        }

        if ($user->stripe_id === null) {
            // Create a new Stripe customer for the user
            $user->createAsStripeCustomer();
        }

        return $user->redirectToBillingPortal();
    }
}
