import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition, applyUrlDefaults } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\L402PaywallController::store
* @see app/Http/Controllers/Api/L402PaywallController.php:32
* @route '/api/l402/paywalls'
*/
export const store = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

store.definition = {
    methods: ["post"],
    url: '/api/l402/paywalls',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\L402PaywallController::store
* @see app/Http/Controllers/Api/L402PaywallController.php:32
* @route '/api/l402/paywalls'
*/
store.url = (options?: RouteQueryOptions) => {
    return store.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\L402PaywallController::store
* @see app/Http/Controllers/Api/L402PaywallController.php:32
* @route '/api/l402/paywalls'
*/
store.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\L402PaywallController::store
* @see app/Http/Controllers/Api/L402PaywallController.php:32
* @route '/api/l402/paywalls'
*/
const storeForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\L402PaywallController::store
* @see app/Http/Controllers/Api/L402PaywallController.php:32
* @route '/api/l402/paywalls'
*/
storeForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

store.form = storeForm

/**
* @see \App\Http\Controllers\Api\L402PaywallController::update
* @see app/Http/Controllers/Api/L402PaywallController.php:90
* @route '/api/l402/paywalls/{paywallId}'
*/
export const update = (args: { paywallId: string | number } | [paywallId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'patch'> => ({
    url: update.url(args, options),
    method: 'patch',
})

update.definition = {
    methods: ["patch"],
    url: '/api/l402/paywalls/{paywallId}',
} satisfies RouteDefinition<["patch"]>

/**
* @see \App\Http\Controllers\Api\L402PaywallController::update
* @see app/Http/Controllers/Api/L402PaywallController.php:90
* @route '/api/l402/paywalls/{paywallId}'
*/
update.url = (args: { paywallId: string | number } | [paywallId: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { paywallId: args }
    }

    if (Array.isArray(args)) {
        args = {
            paywallId: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        paywallId: args.paywallId,
    }

    return update.definition.url
            .replace('{paywallId}', parsedArgs.paywallId.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\L402PaywallController::update
* @see app/Http/Controllers/Api/L402PaywallController.php:90
* @route '/api/l402/paywalls/{paywallId}'
*/
update.patch = (args: { paywallId: string | number } | [paywallId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'patch'> => ({
    url: update.url(args, options),
    method: 'patch',
})

/**
* @see \App\Http\Controllers\Api\L402PaywallController::update
* @see app/Http/Controllers/Api/L402PaywallController.php:90
* @route '/api/l402/paywalls/{paywallId}'
*/
const updateForm = (args: { paywallId: string | number } | [paywallId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: update.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'PATCH',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\L402PaywallController::update
* @see app/Http/Controllers/Api/L402PaywallController.php:90
* @route '/api/l402/paywalls/{paywallId}'
*/
updateForm.patch = (args: { paywallId: string | number } | [paywallId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: update.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'PATCH',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

update.form = updateForm

/**
* @see \App\Http\Controllers\Api\L402PaywallController::destroy
* @see app/Http/Controllers/Api/L402PaywallController.php:157
* @route '/api/l402/paywalls/{paywallId}'
*/
export const destroy = (args: { paywallId: string | number } | [paywallId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'delete'> => ({
    url: destroy.url(args, options),
    method: 'delete',
})

destroy.definition = {
    methods: ["delete"],
    url: '/api/l402/paywalls/{paywallId}',
} satisfies RouteDefinition<["delete"]>

/**
* @see \App\Http\Controllers\Api\L402PaywallController::destroy
* @see app/Http/Controllers/Api/L402PaywallController.php:157
* @route '/api/l402/paywalls/{paywallId}'
*/
destroy.url = (args: { paywallId: string | number } | [paywallId: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { paywallId: args }
    }

    if (Array.isArray(args)) {
        args = {
            paywallId: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        paywallId: args.paywallId,
    }

    return destroy.definition.url
            .replace('{paywallId}', parsedArgs.paywallId.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\L402PaywallController::destroy
* @see app/Http/Controllers/Api/L402PaywallController.php:157
* @route '/api/l402/paywalls/{paywallId}'
*/
destroy.delete = (args: { paywallId: string | number } | [paywallId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'delete'> => ({
    url: destroy.url(args, options),
    method: 'delete',
})

/**
* @see \App\Http\Controllers\Api\L402PaywallController::destroy
* @see app/Http/Controllers/Api/L402PaywallController.php:157
* @route '/api/l402/paywalls/{paywallId}'
*/
const destroyForm = (args: { paywallId: string | number } | [paywallId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: destroy.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'DELETE',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\L402PaywallController::destroy
* @see app/Http/Controllers/Api/L402PaywallController.php:157
* @route '/api/l402/paywalls/{paywallId}'
*/
destroyForm.delete = (args: { paywallId: string | number } | [paywallId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: destroy.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'DELETE',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

destroy.form = destroyForm

const L402PaywallController = { store, update, destroy }

export default L402PaywallController