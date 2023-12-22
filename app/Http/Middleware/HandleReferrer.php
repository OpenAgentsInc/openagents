<?php

namespace App\Http\Middleware;

use Closure;
use Illuminate\Http\Request;
use Symfony\Component\HttpFoundation\Response;

class HandleReferrer
{
    /**
     * Handle an incoming request.
     *
     * @param  \Closure(\Illuminate\Http\Request): (\Symfony\Component\HttpFoundation\Response)  $next
     */
    public function handle(Request $request, Closure $next): Response
    {
        $uri = $request->server->get('REQUEST_URI');

        if (\strpos($uri, "?r=") !== false) {
            $ref = $request->query->get('r');
            $request->session()->put('r', $ref);
            $urlminusquerystring = explode('?', $uri)[0];
            return redirect($urlminusquerystring);
        }

        return $next($request);
    }
}
