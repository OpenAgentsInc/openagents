import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../wayfinder'
import transactions from './transactions'
/**
* @see \App\Http\Controllers\L402PageController::wallet
* @see app/Http/Controllers/L402PageController.php:12
* @route '/l402'
*/
export const wallet = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: wallet.url(options),
    method: 'get',
})

wallet.definition = {
    methods: ["get","head"],
    url: '/l402',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\L402PageController::wallet
* @see app/Http/Controllers/L402PageController.php:12
* @route '/l402'
*/
wallet.url = (options?: RouteQueryOptions) => {
    return wallet.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\L402PageController::wallet
* @see app/Http/Controllers/L402PageController.php:12
* @route '/l402'
*/
wallet.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: wallet.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::wallet
* @see app/Http/Controllers/L402PageController.php:12
* @route '/l402'
*/
wallet.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: wallet.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\L402PageController::wallet
* @see app/Http/Controllers/L402PageController.php:12
* @route '/l402'
*/
const walletForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: wallet.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::wallet
* @see app/Http/Controllers/L402PageController.php:12
* @route '/l402'
*/
walletForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: wallet.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::wallet
* @see app/Http/Controllers/L402PageController.php:12
* @route '/l402'
*/
walletForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: wallet.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

wallet.form = walletForm

/**
* @see \App\Http\Controllers\L402PageController::paywalls
* @see app/Http/Controllers/L402PageController.php:94
* @route '/l402/paywalls'
*/
export const paywalls = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: paywalls.url(options),
    method: 'get',
})

paywalls.definition = {
    methods: ["get","head"],
    url: '/l402/paywalls',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\L402PageController::paywalls
* @see app/Http/Controllers/L402PageController.php:94
* @route '/l402/paywalls'
*/
paywalls.url = (options?: RouteQueryOptions) => {
    return paywalls.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\L402PageController::paywalls
* @see app/Http/Controllers/L402PageController.php:94
* @route '/l402/paywalls'
*/
paywalls.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: paywalls.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::paywalls
* @see app/Http/Controllers/L402PageController.php:94
* @route '/l402/paywalls'
*/
paywalls.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: paywalls.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\L402PageController::paywalls
* @see app/Http/Controllers/L402PageController.php:94
* @route '/l402/paywalls'
*/
const paywallsForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: paywalls.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::paywalls
* @see app/Http/Controllers/L402PageController.php:94
* @route '/l402/paywalls'
*/
paywallsForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: paywalls.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::paywalls
* @see app/Http/Controllers/L402PageController.php:94
* @route '/l402/paywalls'
*/
paywallsForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: paywalls.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

paywalls.form = paywallsForm

/**
* @see \App\Http\Controllers\L402PageController::settlements
* @see app/Http/Controllers/L402PageController.php:142
* @route '/l402/settlements'
*/
export const settlements = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: settlements.url(options),
    method: 'get',
})

settlements.definition = {
    methods: ["get","head"],
    url: '/l402/settlements',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\L402PageController::settlements
* @see app/Http/Controllers/L402PageController.php:142
* @route '/l402/settlements'
*/
settlements.url = (options?: RouteQueryOptions) => {
    return settlements.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\L402PageController::settlements
* @see app/Http/Controllers/L402PageController.php:142
* @route '/l402/settlements'
*/
settlements.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: settlements.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::settlements
* @see app/Http/Controllers/L402PageController.php:142
* @route '/l402/settlements'
*/
settlements.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: settlements.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\L402PageController::settlements
* @see app/Http/Controllers/L402PageController.php:142
* @route '/l402/settlements'
*/
const settlementsForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: settlements.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::settlements
* @see app/Http/Controllers/L402PageController.php:142
* @route '/l402/settlements'
*/
settlementsForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: settlements.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::settlements
* @see app/Http/Controllers/L402PageController.php:142
* @route '/l402/settlements'
*/
settlementsForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: settlements.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

settlements.form = settlementsForm

/**
* @see \App\Http\Controllers\L402PageController::deployments
* @see app/Http/Controllers/L402PageController.php:184
* @route '/l402/deployments'
*/
export const deployments = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: deployments.url(options),
    method: 'get',
})

deployments.definition = {
    methods: ["get","head"],
    url: '/l402/deployments',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\L402PageController::deployments
* @see app/Http/Controllers/L402PageController.php:184
* @route '/l402/deployments'
*/
deployments.url = (options?: RouteQueryOptions) => {
    return deployments.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\L402PageController::deployments
* @see app/Http/Controllers/L402PageController.php:184
* @route '/l402/deployments'
*/
deployments.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: deployments.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::deployments
* @see app/Http/Controllers/L402PageController.php:184
* @route '/l402/deployments'
*/
deployments.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: deployments.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\L402PageController::deployments
* @see app/Http/Controllers/L402PageController.php:184
* @route '/l402/deployments'
*/
const deploymentsForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: deployments.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::deployments
* @see app/Http/Controllers/L402PageController.php:184
* @route '/l402/deployments'
*/
deploymentsForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: deployments.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::deployments
* @see app/Http/Controllers/L402PageController.php:184
* @route '/l402/deployments'
*/
deploymentsForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: deployments.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

deployments.form = deploymentsForm

const l402 = {
    wallet: Object.assign(wallet, wallet),
    transactions: Object.assign(transactions, transactions),
    paywalls: Object.assign(paywalls, paywalls),
    settlements: Object.assign(settlements, settlements),
    deployments: Object.assign(deployments, deployments),
}

export default l402