import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../wayfinder'
/**
* @see \Laravel\Sanctum\Http\Controllers\CsrfCookieController::csrfCookie
* @see vendor/laravel/sanctum/src/Http/Controllers/CsrfCookieController.php:17
* @route '/sanctum/csrf-cookie'
*/
export const csrfCookie = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: csrfCookie.url(options),
    method: 'get',
})

csrfCookie.definition = {
    methods: ["get","head"],
    url: '/sanctum/csrf-cookie',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \Laravel\Sanctum\Http\Controllers\CsrfCookieController::csrfCookie
* @see vendor/laravel/sanctum/src/Http/Controllers/CsrfCookieController.php:17
* @route '/sanctum/csrf-cookie'
*/
csrfCookie.url = (options?: RouteQueryOptions) => {
    return csrfCookie.definition.url + queryParams(options)
}

/**
* @see \Laravel\Sanctum\Http\Controllers\CsrfCookieController::csrfCookie
* @see vendor/laravel/sanctum/src/Http/Controllers/CsrfCookieController.php:17
* @route '/sanctum/csrf-cookie'
*/
csrfCookie.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: csrfCookie.url(options),
    method: 'get',
})

/**
* @see \Laravel\Sanctum\Http\Controllers\CsrfCookieController::csrfCookie
* @see vendor/laravel/sanctum/src/Http/Controllers/CsrfCookieController.php:17
* @route '/sanctum/csrf-cookie'
*/
csrfCookie.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: csrfCookie.url(options),
    method: 'head',
})

/**
* @see \Laravel\Sanctum\Http\Controllers\CsrfCookieController::csrfCookie
* @see vendor/laravel/sanctum/src/Http/Controllers/CsrfCookieController.php:17
* @route '/sanctum/csrf-cookie'
*/
const csrfCookieForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: csrfCookie.url(options),
    method: 'get',
})

/**
* @see \Laravel\Sanctum\Http\Controllers\CsrfCookieController::csrfCookie
* @see vendor/laravel/sanctum/src/Http/Controllers/CsrfCookieController.php:17
* @route '/sanctum/csrf-cookie'
*/
csrfCookieForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: csrfCookie.url(options),
    method: 'get',
})

/**
* @see \Laravel\Sanctum\Http\Controllers\CsrfCookieController::csrfCookie
* @see vendor/laravel/sanctum/src/Http/Controllers/CsrfCookieController.php:17
* @route '/sanctum/csrf-cookie'
*/
csrfCookieForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: csrfCookie.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

csrfCookie.form = csrfCookieForm

const sanctum = {
    csrfCookie: Object.assign(csrfCookie, csrfCookie),
}

export default sanctum