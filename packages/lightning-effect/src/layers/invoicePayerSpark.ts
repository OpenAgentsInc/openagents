import { makeInvoicePayerSparkLayer } from "../adapters/invoicePayerSpark.js"

export const makeInvoicePayerSparkLiveLayer = () => makeInvoicePayerSparkLayer()
export const InvoicePayerSparkLiveLayer = makeInvoicePayerSparkLiveLayer()
