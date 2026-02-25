import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\AuthRegisterController::store
* @see app/Http/Controllers/Api/AuthRegisterController.php:32
* @route '/api/auth/register'
*/
export const store = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

store.definition = {
    methods: ["post"],
    url: '/api/auth/register',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\AuthRegisterController::store
* @see app/Http/Controllers/Api/AuthRegisterController.php:32
* @route '/api/auth/register'
*/
store.url = (options?: RouteQueryOptions) => {
    return store.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AuthRegisterController::store
* @see app/Http/Controllers/Api/AuthRegisterController.php:32
* @route '/api/auth/register'
*/
store.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AuthRegisterController::store
* @see app/Http/Controllers/Api/AuthRegisterController.php:32
* @route '/api/auth/register'
*/
const storeForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AuthRegisterController::store
* @see app/Http/Controllers/Api/AuthRegisterController.php:32
* @route '/api/auth/register'
*/
storeForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

store.form = storeForm

const AuthRegisterController = { store }

export default AuthRegisterController