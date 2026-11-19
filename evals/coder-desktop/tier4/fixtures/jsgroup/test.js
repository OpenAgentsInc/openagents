// Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
const assert = require('assert');
const { groupBy } = require('./util');

assert.deepStrictEqual(groupBy([1, 2, 3, 4], (n) => (n % 2 ? 'odd' : 'even')), { odd: [1, 3], even: [2, 4] });
assert.deepStrictEqual(groupBy([], (x) => x), {});
assert.deepStrictEqual(groupBy(['ab', 'c', 'de'], (s) => s.length), { 2: ['ab', 'de'], 1: ['c'] });
console.log('ok');
