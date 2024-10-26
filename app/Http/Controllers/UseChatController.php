<?php

namespace App\Http\Controllers;

use App\AI\BedrockAIGateway;
use App\Config\ModelPricing;
use App\Services\ToolService;
use App\Traits\UsesChat;
use App\Traits\UsesStreaming;
use Illuminate\Http\Request;

class UseChatController extends Controller
{
    use UsesChat, UsesStreaming;

    private $toolService;
    private $gateway;
    private $model;

    public function __construct(ToolService $toolService, BedrockAIGateway $gateway)
    {
        $this->toolService = $toolService;
        $this->gateway = $gateway;
        $this->model = ModelPricing::$defaultChatModel;
    }

    public function chat(Request $request)
    {
        return $this->createChatStream($request);
    }
}
