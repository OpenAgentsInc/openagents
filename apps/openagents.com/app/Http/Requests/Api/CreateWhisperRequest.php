<?php

namespace App\Http\Requests\Api;

use Illuminate\Foundation\Http\FormRequest;

class CreateWhisperRequest extends FormRequest
{
    public function authorize(): bool
    {
        $user = $this->user();
        if (! $user) {
            return false;
        }

        $token = $user->currentAccessToken();

        return ! $token || $token->can('*') || $token->can('whispers:write');
    }

    /**
     * @return array<string, mixed>
     */
    public function rules(): array
    {
        return [
            'body' => ['required', 'string', 'max:5000'],
            'recipientId' => ['nullable', 'integer', 'exists:users,id', 'required_without:recipientHandle', 'prohibits:recipientHandle'],
            'recipientHandle' => ['nullable', 'string', 'max:64', 'regex:/^[a-z0-9:_-]+$/', 'required_without:recipientId', 'prohibits:recipientId'],
        ];
    }

    protected function prepareForValidation(): void
    {
        $handle = $this->input('recipientHandle');
        if (! is_string($handle)) {
            return;
        }

        $normalized = strtolower(trim($handle));

        $this->merge([
            'recipientHandle' => $normalized === '' ? null : $normalized,
        ]);
    }
}
