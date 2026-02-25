import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition, applyUrlDefaults } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\L402Controller::wallet
* @see app/Http/Controllers/Api/L402Controller.php:31
* @route '/api/l402/wallet'
*/
export const wallet = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: wallet.url(options),
    method: 'get',
})

wallet.definition = {
    methods: ["get","head"],
    url: '/api/l402/wallet',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\L402Controller::wallet
* @see app/Http/Controllers/Api/L402Controller.php:31
* @route '/api/l402/wallet'
*/
wallet.url = (options?: RouteQueryOptions) => {
    return wallet.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\L402Controller::wallet
* @see app/Http/Controllers/Api/L402Controller.php:31
* @route '/api/l402/wallet'
*/
wallet.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: wallet.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::wallet
* @see app/Http/Controllers/Api/L402Controller.php:31
* @route '/api/l402/wallet'
*/
wallet.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: wallet.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::wallet
* @see app/Http/Controllers/Api/L402Controller.php:31
* @route '/api/l402/wallet'
*/
const walletForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: wallet.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::wallet
* @see app/Http/Controllers/Api/L402Controller.php:31
* @route '/api/l402/wallet'
*/
walletForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: wallet.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::wallet
* @see app/Http/Controllers/Api/L402Controller.php:31
* @route '/api/l402/wallet'
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
* @see \App\Http\Controllers\Api\L402Controller::transactions
* @see app/Http/Controllers/Api/L402Controller.php:116
* @route '/api/l402/transactions'
*/
export const transactions = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: transactions.url(options),
    method: 'get',
})

transactions.definition = {
    methods: ["get","head"],
    url: '/api/l402/transactions',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\L402Controller::transactions
* @see app/Http/Controllers/Api/L402Controller.php:116
* @route '/api/l402/transactions'
*/
transactions.url = (options?: RouteQueryOptions) => {
    return transactions.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\L402Controller::transactions
* @see app/Http/Controllers/Api/L402Controller.php:116
* @route '/api/l402/transactions'
*/
transactions.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: transactions.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::transactions
* @see app/Http/Controllers/Api/L402Controller.php:116
* @route '/api/l402/transactions'
*/
transactions.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: transactions.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::transactions
* @see app/Http/Controllers/Api/L402Controller.php:116
* @route '/api/l402/transactions'
*/
const transactionsForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: transactions.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::transactions
* @see app/Http/Controllers/Api/L402Controller.php:116
* @route '/api/l402/transactions'
*/
transactionsForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: transactions.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::transactions
* @see app/Http/Controllers/Api/L402Controller.php:116
* @route '/api/l402/transactions'
*/
transactionsForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: transactions.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

transactions.form = transactionsForm

/**
* @see \App\Http\Controllers\Api\L402Controller::transactionShow
* @see app/Http/Controllers/Api/L402Controller.php:154
* @route '/api/l402/transactions/{eventId}'
*/
export const transactionShow = (args: { eventId: string | number } | [eventId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: transactionShow.url(args, options),
    method: 'get',
})

transactionShow.definition = {
    methods: ["get","head"],
    url: '/api/l402/transactions/{eventId}',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\L402Controller::transactionShow
* @see app/Http/Controllers/Api/L402Controller.php:154
* @route '/api/l402/transactions/{eventId}'
*/
transactionShow.url = (args: { eventId: string | number } | [eventId: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { eventId: args }
    }

    if (Array.isArray(args)) {
        args = {
            eventId: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        eventId: args.eventId,
    }

    return transactionShow.definition.url
            .replace('{eventId}', parsedArgs.eventId.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\L402Controller::transactionShow
* @see app/Http/Controllers/Api/L402Controller.php:154
* @route '/api/l402/transactions/{eventId}'
*/
transactionShow.get = (args: { eventId: string | number } | [eventId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: transactionShow.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::transactionShow
* @see app/Http/Controllers/Api/L402Controller.php:154
* @route '/api/l402/transactions/{eventId}'
*/
transactionShow.head = (args: { eventId: string | number } | [eventId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: transactionShow.url(args, options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::transactionShow
* @see app/Http/Controllers/Api/L402Controller.php:154
* @route '/api/l402/transactions/{eventId}'
*/
const transactionShowForm = (args: { eventId: string | number } | [eventId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: transactionShow.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::transactionShow
* @see app/Http/Controllers/Api/L402Controller.php:154
* @route '/api/l402/transactions/{eventId}'
*/
transactionShowForm.get = (args: { eventId: string | number } | [eventId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: transactionShow.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::transactionShow
* @see app/Http/Controllers/Api/L402Controller.php:154
* @route '/api/l402/transactions/{eventId}'
*/
transactionShowForm.head = (args: { eventId: string | number } | [eventId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: transactionShow.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

transactionShow.form = transactionShowForm

/**
* @see \App\Http\Controllers\Api\L402Controller::paywalls
* @see app/Http/Controllers/Api/L402Controller.php:181
* @route '/api/l402/paywalls'
*/
export const paywalls = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: paywalls.url(options),
    method: 'get',
})

paywalls.definition = {
    methods: ["get","head"],
    url: '/api/l402/paywalls',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\L402Controller::paywalls
* @see app/Http/Controllers/Api/L402Controller.php:181
* @route '/api/l402/paywalls'
*/
paywalls.url = (options?: RouteQueryOptions) => {
    return paywalls.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\L402Controller::paywalls
* @see app/Http/Controllers/Api/L402Controller.php:181
* @route '/api/l402/paywalls'
*/
paywalls.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: paywalls.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::paywalls
* @see app/Http/Controllers/Api/L402Controller.php:181
* @route '/api/l402/paywalls'
*/
paywalls.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: paywalls.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::paywalls
* @see app/Http/Controllers/Api/L402Controller.php:181
* @route '/api/l402/paywalls'
*/
const paywallsForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: paywalls.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::paywalls
* @see app/Http/Controllers/Api/L402Controller.php:181
* @route '/api/l402/paywalls'
*/
paywallsForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: paywalls.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::paywalls
* @see app/Http/Controllers/Api/L402Controller.php:181
* @route '/api/l402/paywalls'
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
* @see \App\Http\Controllers\Api\L402Controller::settlements
* @see app/Http/Controllers/Api/L402Controller.php:244
* @route '/api/l402/settlements'
*/
export const settlements = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: settlements.url(options),
    method: 'get',
})

settlements.definition = {
    methods: ["get","head"],
    url: '/api/l402/settlements',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\L402Controller::settlements
* @see app/Http/Controllers/Api/L402Controller.php:244
* @route '/api/l402/settlements'
*/
settlements.url = (options?: RouteQueryOptions) => {
    return settlements.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\L402Controller::settlements
* @see app/Http/Controllers/Api/L402Controller.php:244
* @route '/api/l402/settlements'
*/
settlements.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: settlements.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::settlements
* @see app/Http/Controllers/Api/L402Controller.php:244
* @route '/api/l402/settlements'
*/
settlements.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: settlements.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::settlements
* @see app/Http/Controllers/Api/L402Controller.php:244
* @route '/api/l402/settlements'
*/
const settlementsForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: settlements.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::settlements
* @see app/Http/Controllers/Api/L402Controller.php:244
* @route '/api/l402/settlements'
*/
settlementsForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: settlements.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::settlements
* @see app/Http/Controllers/Api/L402Controller.php:244
* @route '/api/l402/settlements'
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
* @see \App\Http\Controllers\Api\L402Controller::deployments
* @see app/Http/Controllers/Api/L402Controller.php:301
* @route '/api/l402/deployments'
*/
export const deployments = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: deployments.url(options),
    method: 'get',
})

deployments.definition = {
    methods: ["get","head"],
    url: '/api/l402/deployments',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\L402Controller::deployments
* @see app/Http/Controllers/Api/L402Controller.php:301
* @route '/api/l402/deployments'
*/
deployments.url = (options?: RouteQueryOptions) => {
    return deployments.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\L402Controller::deployments
* @see app/Http/Controllers/Api/L402Controller.php:301
* @route '/api/l402/deployments'
*/
deployments.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: deployments.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::deployments
* @see app/Http/Controllers/Api/L402Controller.php:301
* @route '/api/l402/deployments'
*/
deployments.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: deployments.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::deployments
* @see app/Http/Controllers/Api/L402Controller.php:301
* @route '/api/l402/deployments'
*/
const deploymentsForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: deployments.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::deployments
* @see app/Http/Controllers/Api/L402Controller.php:301
* @route '/api/l402/deployments'
*/
deploymentsForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: deployments.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\L402Controller::deployments
* @see app/Http/Controllers/Api/L402Controller.php:301
* @route '/api/l402/deployments'
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

const L402Controller = { wallet, transactions, transactionShow, paywalls, settlements, deployments }

export default L402Controller