<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Illuminate\Routing\Controller;

class BillingController extends Controller
{
    public function pro(Request $request)
    {
        // Check if the referrer is from Stripe (using our custom pay domain)
        $isFromStripe = $request->server('HTTP_REFERER') && str_contains($request->server('HTTP_REFERER'), 'pay.openagents.com');

        if (! auth()->check() || (! auth()->user()->isPro() && ! $isFromStripe)) {
            return redirect('/');
        }

        return view('pro');
    }

    public function stripe_subscribe(Request $request)
    {
        $user = $request->user();
        if (! $user) {
            return redirect('/');
        }

        return $user
            ->newSubscription('default', env('STRIPE_PRO_SUBSCRIPTION_PRICE_ID'))
            ->checkout([
                'success_url' => route('pro'),
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
