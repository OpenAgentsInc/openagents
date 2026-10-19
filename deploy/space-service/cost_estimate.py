#!/usr/bin/env python3
"""Estimate the private SPACE deployment's hourly cost from live list prices.

The operator can't read the billing account or a billing export, so this
multiplies the deployment's measured resource inventory by on-demand list
prices from the public Cloud Billing Catalog API (us-central1, USD). It is an
estimate: no discounts, credits, or usage-based charges such as NAT data,
object operations, transfer, or Cloud Run requests beyond the always-on
instance. Usage: cost_estimate.py [HOURS]
"""
import json
import re
import subprocess
import sys
import urllib.request
from datetime import datetime, timezone

SERVICES = {'compute': '6F81-5844-456A', 'sql': '9662-B51E-5089', 'run': '152E-C115-5142', 'storage': '95FF-2EF5-5EA1', 'networking': 'E505-1604-58F8'}
# (name, service, description regex, quantity, unit note)
LINES = [
    ('control e2-standard-4 cores', 'compute', r'^E2 Instance Core running in Americas$', 4, 'vCPU-hour'),
    ('control e2-standard-4 memory', 'compute', r'^E2 Instance Ram running in Americas$', 16, 'GiB-hour'),
    ('worker n2-standard-8 cores', 'compute', r'^N2 Instance Core running in Americas$', 8, 'vCPU-hour'),
    ('worker n2-standard-8 memory', 'compute', r'^N2 Instance Ram running in Americas$', 32, 'GiB-hour'),
    ('pd-balanced 460 GiB', 'compute', r'^Balanced PD Capacity( in Iowa)?$', 460 / 730, 'GiB-month per hour'),
    ('Cloud NAT gateway, 3 instances', 'networking', r'^Networking Cloud Nat Gateway Uptime$', 3, 'instance-hour'),
    ('Cloud NAT external IP', 'networking', r'^Networking Cloud NAT IP Usage$', 1, 'IP-hour'),
    ('Cloud SQL vCPU', 'sql', r'^Cloud SQL for PostgreSQL: Zonal - vCPU in Americas$', 1, 'vCPU-hour'),
    ('Cloud SQL memory 3.75 GiB', 'sql', r'^Cloud SQL for PostgreSQL: Zonal - RAM in Americas$', 3.75, 'GiB-hour'),
    ('Cloud SQL SSD 20 GiB', 'sql', r'^Cloud SQL for PostgreSQL: Zonal - Standard storage in Americas$', 20 / 730, 'GiB-month per hour'),
    ('Cloud Run always-on 2 vCPU', 'run', r'^Services CPU \(Instance-based billing\) in us-central1$', 2 * 3600, 'vCPU-second'),
    ('Cloud Run always-on 0.625 GiB', 'run', r'^Services Memory \(Instance-based billing\) in us-central1$', 0.625 * 3600, 'GiB-second'),
    ('object storage about 15 GiB', 'storage', r'^Standard Storage US Regional$|^Standard Storage Iowa$', 15 / 730, 'GiB-month per hour'),
]


def token():
    return subprocess.check_output(['gcloud', 'auth', 'print-access-token'], text=True).strip()


def skus(service, auth):
    out, page = [], ''
    for _ in range(80):
        url = f'https://cloudbilling.googleapis.com/v1/services/{service}/skus?pageSize=5000&currencyCode=USD' + (f'&pageToken={page}' if page else '')
        data = json.loads(urllib.request.urlopen(urllib.request.Request(url, headers={'Authorization': 'Bearer ' + auth}), timeout=60).read())
        out += data.get('skus', [])
        page = data.get('nextPageToken')
        if not page:
            return out
    raise RuntimeError('The price catalog exceeded its page bound.')


def unit_price(sku):
    tiers = sku['pricingInfo'][0]['pricingExpression']['tieredRates']
    rate = tiers[-1]['unitPrice'] if len(tiers) > 1 and float(tiers[0]['unitPrice'].get('nanos', 0)) == 0 and int(tiers[0]['unitPrice'].get('units', 0)) == 0 else tiers[0]['unitPrice']
    return int(rate.get('units', 0)) + rate.get('nanos', 0) / 1e9


def main(hours):
    auth = token()
    catalog = {name: skus(sid, auth) for name, sid in SERVICES.items()}
    lines, total = [], 0.0
    for name, service, pattern, quantity, unit in LINES:
        matches = [s for s in catalog[service] if re.search(pattern, s['description'])
                   and ('us-central1' in s.get('serviceRegions', []) or 'global' in s.get('serviceRegions', []))
                   and s['category'].get('usageType') == 'OnDemand']
        if not matches:
            lines.append({'line': name, 'price': None, 'note': 'no matching SKU'}); continue
        price = unit_price(matches[0])
        cost = price * quantity
        total += cost
        lines.append({'line': name, 'sku': matches[0]['skuId'], 'description': matches[0]['description'], 'unit': unit,
                      'unit_price_usd': round(price, 8), 'quantity_per_hour': round(quantity, 6), 'usd_per_hour': round(cost, 6)})
    print(json.dumps({'schema': 'openagents.space.cost-estimate.v1', 'observed_at': datetime.now(timezone.utc).isoformat(),
                      'method': 'deployment inventory x on-demand list prices from the Cloud Billing Catalog API; not billing data',
                      'currency': 'USD', 'usd_per_hour': round(total, 4), 'hours': hours, 'usd_total': round(total * hours, 4),
                      'missing_lines': [l['line'] for l in lines if l.get('price', 0) is None], 'lines': lines}, indent=1))


if __name__ == '__main__':
    main(float(sys.argv[1]) if len(sys.argv) > 1 else 1.0)
