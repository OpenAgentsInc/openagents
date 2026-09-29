const { format } = require("../../lib/format");

test("sorts keys", () => {
  expect(format({ b: "2", a: "1" })).toBe("a=1\nb=2");
});
