#!/usr/bin/env python3
"""Read an authorized snapshot using the platform's temporary callback credential."""
import json
import sys
import urllib.parse
import urllib.request


def summarize(payload):
    api = payload['api']
    company = urllib.parse.quote(api['company_id'], safe='')
    request = urllib.request.Request(api['base_url'].rstrip('/') + '/api/v1/companies/' + company + '/snapshot',
                                     headers={'Authorization': 'Bearer ' + api['token']})
    with urllib.request.urlopen(request, timeout=20) as response:
        snapshot = json.load(response)
    customers = snapshot['records'].get('customer', [])
    return {'paying_customers': sum(row['data'].get('status') == 'paying' for row in customers),
            'operating_net_minor': snapshot['metrics']['net_operating_cashflow_minor'],
            'company_revision': snapshot['company']['revision'], 'as_of': snapshot['metrics']['as_of']}


def main():
    try:
        result = summarize(json.loads(sys.stdin.readline()))
        print(json.dumps(result, ensure_ascii=False))
        return 0
    except Exception as error:
        print(json.dumps({'status': 'failed', 'error': str(error)}, ensure_ascii=False))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
