import { makeInvoicePayerLndLayer, type InvoicePayerLndLayerOptions } from "../adapters/invoicePayerLnd.js"

export const makeInvoicePayerLiveLayer = (options: InvoicePayerLndLayerOptions) =>
  makeInvoicePayerLndLayer(options)
