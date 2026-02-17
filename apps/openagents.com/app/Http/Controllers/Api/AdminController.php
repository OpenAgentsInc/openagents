<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\OpenApi\Responses\DataObjectResponse;
use App\OpenApi\Responses\ForbiddenResponse;
use App\OpenApi\Responses\UnauthorizedResponse;
use App\Support\AdminAccess;
use Illuminate\Http\JsonResponse;
use Vyuldashev\LaravelOpenApi\Attributes as OpenApi;

#[OpenApi\PathItem]
class AdminController extends Controller
{
    /**
     * Read admin access status.
     *
     * Returns admin email allowlist visibility for authenticated admin users.
     */
    #[OpenApi\Operation(tags: ['Admin'])]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ForbiddenResponse::class, statusCode: 403)]
    public function status(): JsonResponse
    {
        return response()->json([
            'data' => [
                'status' => 'ok',
                'adminEmails' => AdminAccess::emails(),
            ],
        ]);
    }
}
