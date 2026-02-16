<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Support\AdminAccess;
use Illuminate\Http\JsonResponse;

class AdminController extends Controller
{
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
