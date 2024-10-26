<?php

namespace App\Services;

use App\Models\Message;
use App\Config\ModelPricing;

class TokenUsageService
{
    public function deductCreditsForTokenUsage(Message $chatMessage)
    {
        $modelId = $chatMessage->model;
        if (!$modelId) {
            return false;
        }
        $pricing = ModelPricing::getPriceById($modelId);
        $providerCentsPerMillionInputTokens = $pricing['input'];
        $providerCentsPerMillionOutputTokens = $pricing['output'];
        $profitMultiplier = 2;

        $inputTokens = $chatMessage->input_tokens;
        $outputTokens = $chatMessage->output_tokens;

        $inputCost = ($providerCentsPerMillionInputTokens * $inputTokens) / 1000000;
        $outputCost = ($providerCentsPerMillionOutputTokens * $outputTokens) / 1000000;
        $totalCost = ($inputCost + $outputCost) * $profitMultiplier;

        // That's cents, now convert to dollars
        $totalCost = $totalCost / 100;

        if ($chatMessage->team) {
            $chatMessage->team->updateCredits(-$totalCost, 'usage', 'Token usage for chat message', $chatMessage);
        } else {
            request()->user()->updateCredits(-$totalCost, 'usage', 'Token usage for chat message', $chatMessage);
        }

        // Set the message credit_cost
        $chatMessage->credit_cost = $totalCost;
        $chatMessage->save();

        return true;
    }
}
