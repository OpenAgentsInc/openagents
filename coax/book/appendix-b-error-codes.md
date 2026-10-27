# Appendix B. Error codes

Every error `coax0` 0.2.0 can report, generated from its source. Codes are
stable: a code keeps its meaning, and a retired code is never reused.
`…` marks where the compiler fills in a name, type or count.

The first two digits give the stage: `E01` lexer, `E02` parser, `E03`
checker, `E04` compile-time evaluation (Chapter 4).


## E01xx: Lexer

| Code | Message and fix text |
| --- | --- |
| `E0101` | block comment is never closed…add '*/' |
| `E0102` | floating-point literals are not supported by coax0 yet |
| `E0103` | number has no digits after its base prefix |
| `E0104` | integer literal does not fit in int (s64) |
| `E0105` | unknown escape '\…'…use one of \n \t \r \0 \\ \" |
| `E0106` | string is never closed…add '"' before the end of the line |
| `E0107` | non-ASCII character outside a string or comment…identifiers are ASCII only (spec §5) |
| `E0108` | unexpected character '…' |

## E02xx: Parser

| Code | Message and fix text |
| --- | --- |
| `E0201` | expected … …, found |
| `E0202` | expected a type, found |
| `E0203` | comparisons cannot be chained…split it with '&&' |
| `E0204` | '…' is reserved and not supported by coax0 yet |
| `E0205` | expected an expression, found |
| `E0206` | local constants and procedures are not supported by coax0 yet |
| `E0207` | only a variable can be assigned to |
| `E0208` | expected a parameter name, found |
| `E0209` | expected a module name string after #import |
| `E0210` | #… is not supported by coax0 yet |
| `E0212` | global variables are not supported by coax0 yet |
| `E0213` | expected '#import' or a declaration 'name :: ...' at the top level |

## E03xx: Checker

| Code | Message and fix text |
| --- | --- |
| `E0301` | type '…' is not supported by coax0 yet…use int, bool or string |
| `E0302` | unknown type '…' |
| `E0303` | '…' is already declared at line |
| `E0304` | '…' is already the name of a procedure…pick another name |
| `E0305` | procedures are not values in coax0…call it: …() |
| `E0306` | '…' is not declared |
| `E0307` | '-' needs an int, found |
| `E0308` | '!' needs a bool, found …compare it: x == 0… |
| `E0309` | needs two ints, found … and …build strings with print's % formatting |
| `E0310` | compares ints, found … and |
| `E0311` | cannot compare … with …there are no implicit conversions in Coax |
| `E0312` | needs two bools, found … and |
| `E0313` | print is in module Basic, which is not imported |
| `E0314` | '…' is a constant, not a procedure |
| `E0315` | procedure '…' is not declared |
| `E0316` | '…' takes … argument |
| `E0317` | argument … of '…' is |
| `E0318` | print needs a format string |
| `E0319` | print's format must be a string literal, so it can be checked |
| `E0320` | format has … '%' slot… |
| `E0321` | this call returns nothing, so it cannot be printed |
| `E0322` | this expression's value is not used…assign it, or remove it |
| `E0323` | the result of '…' (…) is not used |
| `E0324` | this call returns nothing, so it cannot initialize a variable |
| `E0325` | '…' is … but is initialized with |
| `E0326` | needs an int variable, '…' is |
| `E0327` | '…' is …, assigned |
| `E0328` | '…' must return |
| `E0329` | '…' returns nothing, but a value is returned…add '-> |
| `E0330` | '…' returns …, given |
| `E0331` | condition must be bool, found …compare it: x != 0… |
| `E0332` | module Basic is imported twice…remove this import |
| `E0333` | unknown module '…'…coax0 has one module: "Basic" |
| `E0334` | procedure '…' is already declared at line |
| `E0335` | '…' is already a builtin…pick another name |
| `E0336` | the program has no 'main'…add main :: () { ... } |
| `E0337` | 'main' takes no parameters and returns nothing in coax0…main :: () { ... } |
| `E0338` | '…' can reach its end without returning …add a return at the end |
| `E0340` | constant dependency cycle at '…' |
| `E0341` | a constant must have a value |
| `E0342` | '…' is already a constant or intrinsic…pick another name |
| `E0343` | type_name needs exactly one expression |
| `E0344` | constants cannot be assigned |
| `E0345` | duplicate constant '…' |
| `E0346` | #assert requires bool |
| `E0347` | constant type dependency depth exceeds 64 |

## E04xx: Compile-time evaluation

| Code | Message and fix text |
| --- | --- |
| `E0400` | compile-time evaluation exceeded 100000 steps |
| `E0401` | compile-time expression cannot capture runtime value '…' |
| `E0402` | compile-time constant cycle at '…' |
| `E0403` | compile-time dependency depth exceeds 64 |
| `E0404` | compile-time execution cannot perform output or host I/O |
| `E0405` | compile-time read of an uninitialized variable |
| `E0406` | compile-time assertion failed |
| `E0407` | division by zero during compile-time evaluation |
| `E0408` | a constant procedure call requires #run |

## Retired

| Code | Was | Retired in |
| --- | --- | --- |
| `E0211` | only procedures can be declared at the top level | 0.2.0, when top-level constants arrived |
