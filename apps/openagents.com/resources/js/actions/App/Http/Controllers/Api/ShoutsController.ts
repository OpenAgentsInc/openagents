import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\ShoutsController::index
* @see app/Http/Controllers/Api/ShoutsController.php:37
* @route '/api/shouts'
*/
export const index = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: index.url(options),
    method: 'get',
})

index.definition = {
    methods: ["get","head"],
    url: '/api/shouts',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\ShoutsController::index
* @see app/Http/Controllers/Api/ShoutsController.php:37
* @route '/api/shouts'
*/
index.url = (options?: RouteQueryOptions) => {
    return index.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\ShoutsController::index
* @see app/Http/Controllers/Api/ShoutsController.php:37
* @route '/api/shouts'
*/
index.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ShoutsController::index
* @see app/Http/Controllers/Api/ShoutsController.php:37
* @route '/api/shouts'
*/
index.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: index.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\ShoutsController::index
* @see app/Http/Controllers/Api/ShoutsController.php:37
* @route '/api/shouts'
*/
const indexForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ShoutsController::index
* @see app/Http/Controllers/Api/ShoutsController.php:37
* @route '/api/shouts'
*/
indexForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ShoutsController::index
* @see app/Http/Controllers/Api/ShoutsController.php:37
* @route '/api/shouts'
*/
indexForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

index.form = indexForm

/**
* @see \App\Http\Controllers\Api\ShoutsController::zones
* @see app/Http/Controllers/Api/ShoutsController.php:104
* @route '/api/shouts/zones'
*/
export const zones = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: zones.url(options),
    method: 'get',
})

zones.definition = {
    methods: ["get","head"],
    url: '/api/shouts/zones',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\ShoutsController::zones
* @see app/Http/Controllers/Api/ShoutsController.php:104
* @route '/api/shouts/zones'
*/
zones.url = (options?: RouteQueryOptions) => {
    return zones.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\ShoutsController::zones
* @see app/Http/Controllers/Api/ShoutsController.php:104
* @route '/api/shouts/zones'
*/
zones.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: zones.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ShoutsController::zones
* @see app/Http/Controllers/Api/ShoutsController.php:104
* @route '/api/shouts/zones'
*/
zones.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: zones.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\ShoutsController::zones
* @see app/Http/Controllers/Api/ShoutsController.php:104
* @route '/api/shouts/zones'
*/
const zonesForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: zones.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ShoutsController::zones
* @see app/Http/Controllers/Api/ShoutsController.php:104
* @route '/api/shouts/zones'
*/
zonesForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: zones.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ShoutsController::zones
* @see app/Http/Controllers/Api/ShoutsController.php:104
* @route '/api/shouts/zones'
*/
zonesForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: zones.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

zones.form = zonesForm

/**
* @see \App\Http\Controllers\Api\ShoutsController::store
* @see app/Http/Controllers/Api/ShoutsController.php:72
* @route '/api/shouts'
*/
export const store = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

store.definition = {
    methods: ["post"],
    url: '/api/shouts',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\ShoutsController::store
* @see app/Http/Controllers/Api/ShoutsController.php:72
* @route '/api/shouts'
*/
store.url = (options?: RouteQueryOptions) => {
    return store.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\ShoutsController::store
* @see app/Http/Controllers/Api/ShoutsController.php:72
* @route '/api/shouts'
*/
store.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\ShoutsController::store
* @see app/Http/Controllers/Api/ShoutsController.php:72
* @route '/api/shouts'
*/
const storeForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\ShoutsController::store
* @see app/Http/Controllers/Api/ShoutsController.php:72
* @route '/api/shouts'
*/
storeForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

store.form = storeForm

const ShoutsController = { index, zones, store }

export default ShoutsController