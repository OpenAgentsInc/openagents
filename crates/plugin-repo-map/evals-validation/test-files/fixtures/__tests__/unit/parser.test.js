const { parse } = require("../../lib/parser");

test("parses key=value lines", () => {
  expect(parse("a=1\n# comment\nb = two\n")).toEqual({ a: "1", b: "two" });
});

test("refuses a line without =", () => {
  expect(() => parse("nope")).toThrow("no '='");
});
