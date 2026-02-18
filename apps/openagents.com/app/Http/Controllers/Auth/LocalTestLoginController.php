<?php

namespace App\Http\Controllers\Auth;

use App\Http\Controllers\Controller;
use App\Models\User;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Str;

class LocalTestLoginController extends Controller
{
    public function __invoke(Request $request): RedirectResponse
    {
        abort_unless(config('auth.local_test_login.enabled', false), 404);

        $email = strtolower(trim((string) $request->query('email', '')));

        abort_if(! filter_var($email, FILTER_VALIDATE_EMAIL), 422, 'Invalid email.');
        abort_unless($this->isAllowedEmail($email), 403);

        $workosId = 'test_local_'.substr(hash('sha256', $email), 0, 24);
        $defaultName = trim((string) Str::of($email)->before('@')->replace(['.', '-', '_'], ' ')->title());
        $name = trim((string) $request->query('name', $defaultName));

        /** @var User $user */
        $user = User::query()->firstOrNew(['email' => $email]);
        $user->name = $name !== '' ? $name : 'Maintenance Tester';
        $user->workos_id = $workosId;
        $user->avatar = $user->avatar ?: $this->avatarForEmail($email);
        $user->email_verified_at = $user->email_verified_at ?: now();
        $user->save();

        Auth::guard('web')->login($user);

        $request->session()->put('oa_local_test_auth', true);
        $request->session()->regenerate();

        return redirect()->route('home');
    }

    private function isAllowedEmail(string $email): bool
    {
        $allowed = config('auth.local_test_login.allowed_emails', []);

        if (! is_array($allowed) || $allowed === []) {
            return false;
        }

        return in_array($email, $allowed, true);
    }

    private function avatarForEmail(string $email): string
    {
        $hash = md5(strtolower(trim($email)));

        return 'https://www.gravatar.com/avatar/'.$hash.'?d=identicon';
    }
}
