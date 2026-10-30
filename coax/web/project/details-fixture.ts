// Compare the actual Coax project details path with the independent TS fold.
import { escapeDetail, splitAcceptance } from './details'
import { plain } from './tables'
const cases = [
  'first\rsecond',
  'first\r\nsecond',
  '  first\rsecond\r\nthird\nlast  ',
  'first\nsecond',
  'intro\r## Acceptance\r- check one\r## More\rafter',
  'one\r\n\r\ntwo',
  'text | slash \\ **bold** `code`',
]
const prose = (s: string) => s.split('\n').map(l => plain(l).trimEnd()).join('\n').replace(/\*\*/g, '').replace(/`/g, '')
console.log(JSON.stringify(cases.map(input => ({ input, expected: escapeDetail(prose(splitAcceptance(input.replace(/\r\n?/g, '\n')).body)), oracle: escapeDetail(prose(splitAcceptance(input).body)) }))))
