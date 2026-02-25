import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\ConvexTokenController::store
* @see app/Http/Controllers/Api/ConvexTokenController.php:13
* @route '/api/convex/token'
*/
export const store = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

store.definition = {
    methods: ["post"],
    url: '/api/convex/token',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\ConvexTokenController::store
* @see app/Http/Controllers/Api/ConvexTokenController.php:13
* @route '/api/convex/token'
*/
store.url = (options?: RouteQueryOptions) => {
    return store.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\ConvexTokenController::store
* @see app/Http/Controllers/Api/ConvexTokenController.php:13
* @route '/api/convex/token'
*/
store.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\ConvexTokenController::store
* @see app/Http/Controllers/Api/ConvexTokenController.php:13
* @route '/api/convex/token'
*/
const storeForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\ConvexTokenController::store
* @see app/Http/Controllers/Api/ConvexTokenController.php:13
* @route '/api/convex/token'
*/
storeForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

store.form = storeForm

const ConvexTokenController = { store }

export default ConvexTokenController