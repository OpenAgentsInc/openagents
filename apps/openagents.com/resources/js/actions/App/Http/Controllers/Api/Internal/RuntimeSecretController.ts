import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\Internal\RuntimeSecretController::fetch
* @see app/Http/Controllers/Api/Internal/RuntimeSecretController.php:12
* @route '/api/internal/runtime/integrations/secrets/fetch'
*/
export const fetch = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: fetch.url(options),
    method: 'post',
})

fetch.definition = {
    methods: ["post"],
    url: '/api/internal/runtime/integrations/secrets/fetch',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\Internal\RuntimeSecretController::fetch
* @see app/Http/Controllers/Api/Internal/RuntimeSecretController.php:12
* @route '/api/internal/runtime/integrations/secrets/fetch'
*/
fetch.url = (options?: RouteQueryOptions) => {
    return fetch.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\Internal\RuntimeSecretController::fetch
* @see app/Http/Controllers/Api/Internal/RuntimeSecretController.php:12
* @route '/api/internal/runtime/integrations/secrets/fetch'
*/
fetch.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: fetch.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\Internal\RuntimeSecretController::fetch
* @see app/Http/Controllers/Api/Internal/RuntimeSecretController.php:12
* @route '/api/internal/runtime/integrations/secrets/fetch'
*/
const fetchForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: fetch.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\Internal\RuntimeSecretController::fetch
* @see app/Http/Controllers/Api/Internal/RuntimeSecretController.php:12
* @route '/api/internal/runtime/integrations/secrets/fetch'
*/
fetchForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: fetch.url(options),
    method: 'post',
})

fetch.form = fetchForm

const RuntimeSecretController = { fetch }

export default RuntimeSecretController