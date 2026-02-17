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
        $zone = $this->input('zone');
        if (! is_string($zone)) {
            return;
        }

        $zone = strtolower(trim($zone));

        $this->merge([
            'zone' => $zone === '' ? null : $zone,
        ]);
    }
}
