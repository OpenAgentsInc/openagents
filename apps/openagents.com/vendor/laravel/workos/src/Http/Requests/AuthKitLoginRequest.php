<?php

namespace Laravel\WorkOS\Http\Requests;

use Illuminate\Foundation\Http\FormRequest;
use Illuminate\Support\Facades\URL;
use Illuminate\Support\Str;
use Inertia\Inertia;
use Laravel\WorkOS\WorkOS;
use Symfony\Component\HttpFoundation\Response;
use WorkOS\UserManagement;

class AuthKitLoginRequest extends FormRequest
{
    /**
     * Redirect the user to WorkOS for authentication.
     *
     * @param  array{
     *     screenHint?: 'sign-in'|'sign-up',
     *     domainHint?: string,
     *     loginHint?: string,
     *     redirectUrl?: string,
     * }  $options
     */
    public function redirect(array $options = []): Response
    {
        WorkOS::configure();

        $url = (new UserManagement)->getAuthorizationUrl(
            $options['redirectUrl'] ?? config('services.workos.redirect_url'),
            $state = [
                'state' => Str::random(20),
                'previous_url' => base64_encode(URL::previous()),
            ],
            'authkit',
            domainHint: $options['domainHint'] ?? null,
            loginHint: $options['loginHint'] ?? null,
            screenHint: $options['screenHint'] ?? null,
        );

        $this->session()->put('state', json_encode($state));

        return class_exists(Inertia::class)
            ? Inertia::location($url)
            : redirect($url);
    }
}
