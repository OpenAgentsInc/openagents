import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  progress,
  table,
  tableBody,
  tableCaption,
  tableCell,
  tableContainer,
  tableFooter,
  tableHead,
  tableHeader,
  tableRow,
} from './data'
import { renderHtml } from './test-helpers'

describe('basecoat data components', () => {
  test('renders Basecoat table markup with semantic sections and selected rows', () => {
    const rendered = renderHtml(
      tableContainer({
        children: [
          table({
            children: [
              tableCaption({ children: ['A list of invoices.'] }),
              tableHeader({
                children: [
                  tableRow({
                    children: [
                      tableHead({ scope: 'col', children: ['Invoice'] }),
                      tableHead({ scope: 'col', className: 'text-end', children: ['Amount'] }),
                    ],
                  }),
                ],
              }),
              tableBody({
                children: [
                  tableRow({
                    state: 'selected',
                    children: [
                      tableCell({ className: 'font-medium', children: ['INV001'] }),
                      tableCell({ className: 'text-end', children: ['$250.00'] }),
                    ],
                  }),
                ],
              }),
              tableFooter({
                children: [
                  tableRow({
                    children: [
                      tableCell({ colSpan: 1, children: ['Total'] }),
                      tableCell({ className: 'text-end', children: ['$250.00'] }),
                    ],
                  }),
                ],
              }),
            ],
          }),
        ],
      }),
    )

    expect(rendered).toContain('<div class="table-container">')
    expect(rendered).toContain('<table class="table">')
    expect(rendered).toContain('<caption>A list of invoices.</caption>')
    expect(rendered).toContain('<thead>')
    expect(rendered).toContain('<th scope="col">Invoice</th>')
    expect(rendered).toContain('<th scope="col" class="text-end">Amount</th>')
    expect(rendered).toContain('<tbody>')
    expect(rendered).toContain('<tr data-state="selected">')
    expect(rendered).toContain('<td class="font-medium">INV001</td>')
    expect(rendered).toContain('<tfoot>')
    expect(rendered).toContain('<td colspan="1">Total</td>')
  })

  test('renders Basecoat progress markup with ARIA values and indicator width', () => {
    const rendered = renderHtml(
      progress({
        value: 66,
        label: 'Loading',
      }),
    )

    expect(rendered).toContain('<div')
    expect(rendered).toContain('class="progress"')
    expect(rendered).toContain('role="progressbar"')
    expect(rendered).toContain('aria-label="Loading"')
    expect(rendered).toContain('aria-valuenow="66"')
    expect(rendered).toContain('aria-valuemin="0"')
    expect(rendered).toContain('aria-valuemax="100"')
    expect(rendered).toContain('<span style="width: 66%"></span>')
  })

  test('clamps progress indicator width while preserving the announced value', () => {
    const rendered = renderHtml(
      progress({
        value: 150,
        max: 120,
        labelledBy: 'progress-label',
      }),
    )

    expect(rendered).toContain('aria-labelledby="progress-label"')
    expect(rendered).toContain('aria-valuenow="150"')
    expect(rendered).toContain('aria-valuemax="120"')
    expect(rendered).toContain('<span style="width: 100%"></span>')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.tableContainer).toBe(tableContainer)
    expect(Basecoat.table).toBe(table)
    expect(Basecoat.tableCaption).toBe(tableCaption)
    expect(Basecoat.tableHeader).toBe(tableHeader)
    expect(Basecoat.tableBody).toBe(tableBody)
    expect(Basecoat.tableFooter).toBe(tableFooter)
    expect(Basecoat.tableRow).toBe(tableRow)
    expect(Basecoat.tableHead).toBe(tableHead)
    expect(Basecoat.tableCell).toBe(tableCell)
    expect(Basecoat.progress).toBe(progress)
  })
})
