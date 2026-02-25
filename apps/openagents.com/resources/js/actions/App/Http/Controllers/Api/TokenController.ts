import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition, applyUrlDefaults } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\TokenController::index
* @see app/Http/Controllers/Api/TokenController.php:32
* @route '/api/tokens'
*/
export const index = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: index.url(options),
    method: 'get',
})

index.definition = {
    methods: ["get","head"],
    url: '/api/tokens',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\TokenController::index
* @see app/Http/Controllers/Api/TokenController.php:32
* @route '/api/tokens'
*/
index.url = (options?: RouteQueryOptions) => {
    return index.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\TokenController::index
* @see app/Http/Controllers/Api/TokenController.php:32
* @route '/api/tokens'
*/
index.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\TokenController::index
* @see app/Http/Controllers/Api/TokenController.php:32
* @route '/api/tokens'
*/
index.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: index.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\TokenController::index
* @see app/Http/Controllers/Api/TokenController.php:32
* @route '/api/tokens'
*/
const indexForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\TokenController::index
* @see app/Http/Controllers/Api/TokenController.php:32
* @route '/api/tokens'
*/
indexForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\TokenController::index
* @see app/Http/Controllers/Api/TokenController.php:32
* @route '/api/tokens'
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
* @see \App\Http\Controllers\Api\TokenController::store
* @see app/Http/Controllers/Api/TokenController.php:70
* @route '/api/tokens'
*/
export const store = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

store.definition = {
    methods: ["post"],
    url: '/api/tokens',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\TokenController::store
* @see app/Http/Controllers/Api/TokenController.php:70
* @route '/api/tokens'
*/
store.url = (options?: RouteQueryOptions) => {
    return store.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\TokenController::store
* @see app/Http/Controllers/Api/TokenController.php:70
* @route '/api/tokens'
*/
store.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\TokenController::store
* @see app/Http/Controllers/Api/TokenController.php:70
* @route '/api/tokens'
*/
const storeForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\TokenController::store
* @see app/Http/Controllers/Api/TokenController.php:70
* @route '/api/tokens'
*/
storeForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

store.form = storeForm

/**
* @see \App\Http\Controllers\Api\TokenController::destroyCurrent
* @see app/Http/Controllers/Api/TokenController.php:142
* @route '/api/tokens/current'
*/
export const destroyCurrent = (options?: RouteQueryOptions): RouteDefinition<'delete'> => ({
    url: destroyCurrent.url(options),
    method: 'delete',
})

destroyCurrent.definition = {
    methods: ["delete"],
    url: '/api/tokens/current',
} satisfies RouteDefinition<["delete"]>

/**
* @see \App\Http\Controllers\Api\TokenController::destroyCurrent
* @see app/Http/Controllers/Api/TokenController.php:142
* @route '/api/tokens/current'
*/
destroyCurrent.url = (options?: RouteQueryOptions) => {
    return destroyCurrent.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\TokenController::destroyCurrent
* @see app/Http/Controllers/Api/TokenController.php:142
* @route '/api/tokens/current'
*/
destroyCurrent.delete = (options?: RouteQueryOptions): RouteDefinition<'delete'> => ({
    url: destroyCurrent.url(options),
    method: 'delete',
})

/**
* @see \App\Http\Controllers\Api\TokenController::destroyCurrent
* @see app/Http/Controllers/Api/TokenController.php:142
* @route '/api/tokens/current'
*/
const destroyCurrentForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: destroyCurrent.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'DELETE',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\TokenController::destroyCurrent
* @see app/Http/Controllers/Api/TokenController.php:142
* @route '/api/tokens/current'
*/
destroyCurrentForm.delete = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: destroyCurrent.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'DELETE',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

destroyCurrent.form = destroyCurrentForm

/**
* @see \App\Http\Controllers\Api\TokenController::destroy
* @see app/Http/Controllers/Api/TokenController.php:120
* @route '/api/tokens/{tokenId}'
*/
export const destroy = (args: { tokenId: string | number } | [tokenId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'delete'> => ({
    url: destroy.url(args, options),
    method: 'delete',
})

destroy.definition = {
    methods: ["delete"],
    url: '/api/tokens/{tokenId}',
} satisfies RouteDefinition<["delete"]>

/**
* @see \App\Http\Controllers\Api\TokenController::destroy
* @see app/Http/Controllers/Api/TokenController.php:120
* @route '/api/tokens/{tokenId}'
*/
destroy.url = (args: { tokenId: string | number } | [tokenId: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { tokenId: args }
    }

    if (Array.isArray(args)) {
        args = {
            tokenId: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        tokenId: args.tokenId,
    }

    return destroy.definition.url
            .replace('{tokenId}', parsedArgs.tokenId.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\TokenController::destroy
* @see app/Http/Controllers/Api/TokenController.php:120
* @route '/api/tokens/{tokenId}'
*/
destroy.delete = (args: { tokenId: string | number } | [tokenId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'delete'> => ({
    url: destroy.url(args, options),
    method: 'delete',
})

/**
* @see \App\Http\Controllers\Api\TokenController::destroy
* @see app/Http/Controllers/Api/TokenController.php:120
* @route '/api/tokens/{tokenId}'
*/
const destroyForm = (args: { tokenId: string | number } | [tokenId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: destroy.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'DELETE',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\TokenController::destroy
* @see app/Http/Controllers/Api/TokenController.php:120
* @route '/api/tokens/{tokenId}'
*/
destroyForm.delete = (args: { tokenId: string | number } | [tokenId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: destroy.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'DELETE',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

destroy.form = destroyForm

/**
* @see \App\Http\Controllers\Api\TokenController::destroyAll
* @see app/Http/Controllers/Api/TokenController.php:175
* @route '/api/tokens'
*/
export const destroyAll = (options?: RouteQueryOptions): RouteDefinition<'delete'> => ({
    url: destroyAll.url(options),
    method: 'delete',
})

destroyAll.definition = {
    methods: ["delete"],
    url: '/api/tokens',
} satisfies RouteDefinition<["delete"]>

/**
* @see \App\Http\Controllers\Api\TokenController::destroyAll
* @see app/Http/Controllers/Api/TokenController.php:175
* @route '/api/tokens'
*/
destroyAll.url = (options?: RouteQueryOptions) => {
    return destroyAll.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\TokenController::destroyAll
* @see app/Http/Controllers/Api/TokenController.php:175
* @route '/api/tokens'
*/
destroyAll.delete = (options?: RouteQueryOptions): RouteDefinition<'delete'> => ({
    url: destroyAll.url(options),
    method: 'delete',
})

/**
* @see \App\Http\Controllers\Api\TokenController::destroyAll
* @see app/Http/Controllers/Api/TokenController.php:175
* @route '/api/tokens'
*/
const destroyAllForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: destroyAll.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'DELETE',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\TokenController::destroyAll
* @see app/Http/Controllers/Api/TokenController.php:175
* @route '/api/tokens'
*/
destroyAllForm.delete = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: destroyAll.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'DELETE',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

destroyAll.form = destroyAllForm

const TokenController = { index, store, destroyCurrent, destroy, destroyAll }

export default TokenController