import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\SpacetimeTokenController::store
* @see app/Http/Controllers/Api/SpacetimeTokenController.php:13
* @route '/api/sync/token'
*/
export const store = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

store.definition = {
    methods: ["post"],
    url: '/api/sync/token',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\SpacetimeTokenController::store
* @see app/Http/Controllers/Api/SpacetimeTokenController.php:13
* @route '/api/sync/token'
*/
store.url = (options?: RouteQueryOptions) => {
    return store.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\SpacetimeTokenController::store
* @see app/Http/Controllers/Api/SpacetimeTokenController.php:13
* @route '/api/sync/token'
*/
store.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\SpacetimeTokenController::store
* @see app/Http/Controllers/Api/SpacetimeTokenController.php:13
* @route '/api/sync/token'
*/
const storeForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\SpacetimeTokenController::store
* @see app/Http/Controllers/Api/SpacetimeTokenController.php:13
* @route '/api/sync/token'
*/
storeForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

store.form = storeForm

const SpacetimeTokenController = { store }

export default SpacetimeTokenController
