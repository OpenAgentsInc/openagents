import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../wayfinder'
/**
* @see \App\Http\Controllers\Auth\LocalTestLoginController::__invoke
* @see app/Http/Controllers/Auth/LocalTestLoginController.php:14
* @route '/internal/test-login'
*/
export const testLogin = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: testLogin.url(options),
    method: 'get',
})

testLogin.definition = {
    methods: ["get","head"],
    url: '/internal/test-login',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Auth\LocalTestLoginController::__invoke
* @see app/Http/Controllers/Auth/LocalTestLoginController.php:14
* @route '/internal/test-login'
*/
testLogin.url = (options?: RouteQueryOptions) => {
    return testLogin.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Auth\LocalTestLoginController::__invoke
* @see app/Http/Controllers/Auth/LocalTestLoginController.php:14
* @route '/internal/test-login'
*/
testLogin.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: testLogin.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Auth\LocalTestLoginController::__invoke
* @see app/Http/Controllers/Auth/LocalTestLoginController.php:14
* @route '/internal/test-login'
*/
testLogin.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: testLogin.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Auth\LocalTestLoginController::__invoke
* @see app/Http/Controllers/Auth/LocalTestLoginController.php:14
* @route '/internal/test-login'
*/
const testLoginForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: testLogin.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Auth\LocalTestLoginController::__invoke
* @see app/Http/Controllers/Auth/LocalTestLoginController.php:14
* @route '/internal/test-login'
*/
testLoginForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: testLogin.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Auth\LocalTestLoginController::__invoke
* @see app/Http/Controllers/Auth/LocalTestLoginController.php:14
* @route '/internal/test-login'
*/
testLoginForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: testLogin.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

testLogin.form = testLoginForm

const internal = {
    testLogin: Object.assign(testLogin, testLogin),
}

export default internal