const { parse } = require("../../lib/parser");
const { format } = require("../../lib/format");

test("round trips through the command line's format", () => {
  const text = "name=demo\nport=8080";
  expect(format(parse(text))).toBe(text);
});
