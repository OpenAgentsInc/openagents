<?php

use Illuminate\Database\Eloquent\ModelNotFoundException;
use Illuminate\Database\QueryException;
use Illuminate\Foundation\Application;
use Illuminate\Foundation\Configuration\Exceptions;
use Illuminate\Foundation\Configuration\Middleware;
use Illuminate\Http\Request;
use Illuminate\Session\TokenMismatchException;
use Symfony\Component\HttpKernel\Exception\MethodNotAllowedHttpException;
use Symfony\Component\HttpKernel\Exception\NotFoundHttpException;

return Application::configure(basePath: dirname(__DIR__))
    ->withRouting(
        web: __DIR__.'/../routes/web.php',
        api: __DIR__.'/../routes/api.php',
        commands: __DIR__.'/../routes/console.php',
        health: '/up',
    )
    ->withMiddleware(function (Middleware $middleware) {
        $middleware->validateCsrfTokens(except: [
            'stripe/*',
            'webhook/nostr', // DEPRECATED
            'webhook/pool',
        ]);
    })
    ->withExceptions(function (Exceptions $exceptions) {
        $exceptions->render(function (TokenMismatchException $e, Request $request) {
            if ($request->expectsJson()) {
                return response()->json([
                    'status' => 'error',
                    'message' => 'Token mismatch',
                    'data' => null,
                    'error' => 'Your session has expired. Please login and try again.',
                ], 419);
            }

        });

        $exceptions->render(function (NotFoundHttpException $e, Request $request) {

            if ($request->expectsJson()) {
                return response()->json([
                    'status' => 'error',
                    'message' => 'Resource not found',
                    'data' => null,
                    'error' => 'The requested resource could not be found on this server.',
                ], 404);
            }
        });

        $exceptions->render(function (ModelNotFoundException $e, Request $request) {

            if ($request->expectsJson()) {
                return response()->json([
                    'status' => 'error',
                    'message' => 'No record found',
                    'data' => null,
                    'error' => 'No records found for the query',
                ], 404);
            }
        });

        $exceptions->render(function (QueryException $e, Request $request) {

            if ($request->expectsJson()) {
                return response()->json([
                    'status' => 'error',
                    'message' => 'Query error',
                    'data' => null,
                    'error' => 'There was an error executing a query.',
                ], 500);
            }
        });

        $exceptions->render(function (PDOException $e, Request $request) {
            if ($request->expectsJson()) {
                return response()->json([
                    'status' => 'error',
                    'message' => 'Database connection error',
                    'data' => null,
                    'error' => 'There was an error establishing connection.',
                ], 500);
            }
        });

        $exceptions->render(function (MethodNotAllowedHttpException $e, Request $request) {
            if ($request->expectsJson()) {
                return response()->json([
                    'status' => 'error',
                    'message' => 'Method not allowed',
                    'data' => null,
                    'error' => 'The requested HTTP method is not allowed for this URL.',
                ], 405);
            }
        });

    })
    ->create();
