import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Auth\LocalTestLoginController::__invoke
* @see app/Http/Controllers/Auth/LocalTestLoginController.php:14
* @route '/internal/test-login'
*/
const LocalTestLoginController = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: LocalTestLoginController.url(options),
    method: 'get',
})

LocalTestLoginController.definition = {
    methods: ["get","head"],
    url: '/internal/test-login',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Auth\LocalTestLoginController::__invoke
* @see app/Http/Controllers/Auth/LocalTestLoginController.php:14
* @route '/internal/test-login'
*/
LocalTestLoginController.url = (options?: RouteQueryOptions) => {
    return LocalTestLoginController.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Auth\LocalTestLoginController::__invoke
* @see app/Http/Controllers/Auth/LocalTestLoginController.php:14
* @route '/internal/test-login'
*/
LocalTestLoginController.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: LocalTestLoginController.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Auth\LocalTestLoginController::__invoke
* @see app/Http/Controllers/Auth/LocalTestLoginController.php:14
* @route '/internal/test-login'
*/
LocalTestLoginController.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: LocalTestLoginController.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Auth\LocalTestLoginController::__invoke
* @see app/Http/Controllers/Auth/LocalTestLoginController.php:14
* @route '/internal/test-login'
*/
const LocalTestLoginControllerForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: LocalTestLoginController.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Auth\LocalTestLoginController::__invoke
* @see app/Http/Controllers/Auth/LocalTestLoginController.php:14
* @route '/internal/test-login'
*/
LocalTestLoginControllerForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: LocalTestLoginController.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Auth\LocalTestLoginController::__invoke
* @see app/Http/Controllers/Auth/LocalTestLoginController.php:14
* @route '/internal/test-login'
*/
LocalTestLoginControllerForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: LocalTestLoginController.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

LocalTestLoginController.form = LocalTestLoginControllerForm

export default LocalTestLoginController