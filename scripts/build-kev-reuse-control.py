"""Build an unscored fixed-document control for the encoded-state reuse audit."""
import hashlib
import json

items = []
for length, copies in [('short', 2), ('long', 48)]:
    for mode in ['repeated', 'new']:
        for index in range(20):
            alternate = index % 2 == 1
            state = 'The customer received the wrong size shoes and requests an exchange. ' * copies
            if mode == 'new':
                state += f' Ticket reference: {index:04d}.'
            question = ({'type': 'choice', 'instructions': 'Does the customer request an exchange?',
                         'criteria': {'yes': 'An exchange is requested', 'no': 'No exchange is requested'}}
                        if alternate else
                        {'type': 'choice', 'instructions': 'Which team should handle the request?',
                         'criteria': {'returns': 'Exchanges and refunds', 'shipping': 'Delivery delays', 'billing': 'Payment problems'}})
            items.append({'id': f'{length}/{mode}/{index}',
                          'family': f'{length}-{mode}-' + ('exchange' if alternate else 'routing'),
                          'kind': 'choice', 'state': state, 'question': question,
                          'truth': 'yes' if alternate else 'returns', 'partition': 'calibration' if length == 'short' else 'development'})
# Gym requires all partitions. This synthetic sentinel is never encoded or scored;
# it is not a held-out quality example or part of the 80-request reuse control.
items.append({'id': 'unscored-schema-sentinel', 'family': 'sentinel',
              'kind': 'choice', 'state': 'Synthetic schema sentinel.',
              'question': {'type': 'choice', 'instructions': 'Is this synthetic?',
                           'criteria': {'yes': 'Synthetic', 'no': 'Real'}},
              'truth': 'yes', 'partition': 'locked'})
suite = {'schema': 'openagents.gym.suite.v1', 'name': 'state-reuse-control',
         'created': '2026-09-20', 'description': 'Unscored encoding control: alternating questions over fixed or changing synthetic states.',
         'items': items,
         'digest': hashlib.sha256(json.dumps(items, sort_keys=True, ensure_ascii=False, separators=(',', ':')).encode()).hexdigest()}
print(json.dumps(suite, indent=2))
