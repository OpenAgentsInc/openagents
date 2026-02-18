<?php

namespace App\Http\Requests\Api;

use Illuminate\Foundation\Http\FormRequest;

class CreateShoutRequest extends FormRequest
{
    public function authorize(): bool
    {
        $user = $this->user();
        if (! $user) {
            return false;
        }

        $token = $user->currentAccessToken();

        return ! $token || $token->can('*') || $token->can('shouts:write');
    }

    /**
     * @return array<string, mixed>
     */
    public function rules(): array
    {
        return [
            'body' => ['required', 'string', 'max:2000'],
            'zone' => ['nullable', 'string', 'max:64', 'regex:/^[a-z0-9:_-]+$/'],
        ];
    }

    protected function prepareForValidation(): void
    {
        $merged = [];

        $zone = $this->input('zone');
        if (is_string($zone)) {
            $normalizedZone = strtolower(trim($zone));
            $merged['zone'] = $normalizedZone === '' ? null : $normalizedZone;
        }

        $body = $this->input('body');
        if (! is_string($body) || trim($body) === '') {
            $textAlias = $this->input('text');
            if (is_string($textAlias) && trim($textAlias) !== '') {
                $body = $textAlias;
            }
        }

        if (is_string($body)) {
            $merged['body'] = trim($body);
        }

        if ($merged !== []) {
            $this->merge($merged);
        }
    }
}
