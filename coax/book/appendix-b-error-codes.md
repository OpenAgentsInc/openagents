# Appendix B. Error codes

<!-- coax-release:current:start -->
Current compiler: **coax0 0.14.0**. Status inventory schema: **1**.
<!-- coax-release:current:end -->
This appendix lists every error code the current compiler can report. The tables
below are the output of `coax explain --all --markdown`, which prints the
compiler's own table from `coax/src/errors.cpp`; when that table changes,
regenerate this appendix from it rather than editing the rows by hand. In
a terminal, `coax explain CODE` prints one entry:

```
$ coax explain E0303
E0303: name already declared; no shadowing
  Every local name in a procedure is unique among the names in scope. Coax has no shadowing, so a name always means one thing.
```

Codes are stable: a code keeps its meaning, and a retired code is never
reused, so the tables include the retired ones too.

The first two digits after the `E` give the stage that reports the error:

| Prefix | Stage | Where to read more |
| --- | --- | --- |
| `E01` | Lexer | Chapter 4 |
| `E02` | Parser | Chapter 4 |
| `E03` | Checker | Chapter 4 |
| `E04` | Compile-time evaluation and checks | Chapters 9 and 10 |
| `E05` | Protobuf descriptor imports | Chapter 15 |
| `E06` | Source-unit composition | Chapter 22 |

Chapter 4 explains how to read a diagnostic, and Chapter 20 shows which
part of the compiler produces each stage's errors. With `--json`, each
diagnostic is one JSON object per line on stderr, with the fields `file`,
`line`, `col`, `code`, `message` and `fix`.


## E01xx: lexer

| Code | Meaning | What to do |
| --- | --- | --- |
| `E0101` | block comment is never closed | A /* comment has no matching */ before the end of the file. Block comments nest, so every /* inside needs its own */. |
| `E0102` | retired in 0.3.0 | Was: floating-point literals are not supported. Floats arrived in 0.3.0. |
| `E0103` | number has no digits after its base prefix | 0x and 0b must be followed by at least one digit of that base. |
| `E0104` | integer literal does not fit in 64 bits | Integer literals go up to 18446744073709551615 (the u64 maximum). Where the literal is used, it must also fit that type. |
| `E0105` | unknown escape in a string | Strings support \n \t \r \0 \\ and \". Unicode and hex escapes are planned. |
| `E0106` | string is never closed | A string ends with a " on the same line. Strings cannot span lines yet. |
| `E0107` | non-ASCII character outside a string or comment | Identifiers and operators are ASCII. UTF-8 text belongs in strings and comments. |
| `E0108` | unexpected character | The character starts no token in Coax. |
| `E0109` | bad floating-point literal | The literal rounds to infinity (the largest f64 is about 1.8e308, the largest f32 about 3.4e38), or it is followed by something other than an f32 or f64 suffix. |
| `E0110` | invalid character literal | A character literal is one ASCII character or escape in single quotes ('a', '\n', '\''). It is an integer literal holding that byte, so it fits u8. |
| `E0111` | lexer source or storage limit | Source must fit signed 32-bit byte offsets and one-based EOF locations. Native lexer buffers must meet the Coax capacity contract. |

## E02xx: parser

| Code | Meaning | What to do |
| --- | --- | --- |
| `E0201` | expected a token | The parser needed a specific token here, usually ';', ')' or '}'. For a missing ';' the caret points right after the previous token. |
| `E0202` | expected a type | A type is a name (int, f32, Vec2), a fixed array '[N] T', or a view '[] T'. |
| `E0203` | comparisons cannot be chained | a < b < c is rejected rather than given a surprising meaning. Write a < b && b < c. |
| `E0204` | reserved word not supported yet | The word is reserved for a planned feature (union, switch, using, try...). coax0 refuses it rather than guessing what it means. |
| `E0205` | expected an expression | A value was needed here: a literal, a name, a call, or a parenthesized expression. |
| `E0206` | local declarations with :: are not supported | Constants, procedures and types are declared at the top level in coax0. |
| `E0207` | cannot assign to this expression | Only a variable, a struct field or an element (a[i]) can be assigned. |
| `E0208` | expected a name | A parameter, field, member or loop variable name was expected here. |
| `E0209` | expected a module name after #import | Write #import "Basic"; |
| `E0210` | directive not supported here | The directive is unknown or not implemented in this position. Procedures accept #export_c; files accept #import and #assert; expressions accept #run. |
| `E0211` | retired in 0.2.0 | Was: only procedures can be declared at the top level. Top-level constants arrived in 0.2.0. |
| `E0212` | global variables are not supported | Top-level mutable state is not part of coax0. Use constants, or pass state explicitly. |
| `E0213` | expected a top-level declaration | The top level holds #import, #assert and declarations 'name :: ...'. |
| `E0214` | unknown cast modifier | The only modifier is trunc: cast,trunc(u8) x keeps the low bits instead of checking that the value fits. |
| `E0215` | struct literal needs a struct name | Write the struct's name before .{ }: Point.{x = 1, y = 2}. Inferred literals are planned. |
| `E0216` | ranges are half-open | Coax ranges are written lo..<hi and include lo but not hi. '..' alone is not a range. |
| `E0217` | array literal needs a fixed array type | Write the type first: [3] int.[1, 2, 3]. |
| `E0218` | a range loop has one variable | for i: 0..<n { } has a counter only. Index variables belong to loops over arrays, views and strings. |
| `E0219` | enum needs an underlying type | An enum names its integer width: Outcome :: enum u8 { ... }. The width is part of its layout. |
| `E0220` | malformed check | #test takes a name string and a block: #test "refunds never exceed payment" { ... }. #forall takes one or more bindings, each a type or a lo..<hi range, then a block: #forall a: u8, b: 0..<10 { ... }. |
| `E0221` | parser resource limit | The Coax parser requires an EOF-terminated lexer token stream, caller-owned indexed-tree storage and at most 128 active recursive grammar entries. Reduce syntax nesting or supply the capacities returned by PS_caps. No partial syntax tree is admitted after a failure. |

## E03xx: checker

| Code | Meaning | What to do |
| --- | --- | --- |
| `E0301` | type not supported yet | The type is in the plan (spec 6.2) but coax0 does not implement it, for example dynamic arrays '[..] T' or vectors. |
| `E0302` | unknown type | No built-in type, struct or enum has this name. |
| `E0303` | name already declared; no shadowing | Every local name in a procedure is unique among the names in scope. Coax has no shadowing, so a name always means one thing. |
| `E0304` | name is already a procedure | A local cannot take the name of a procedure. |
| `E0305` | procedures are not values | Procedure values and callbacks are planned. Call the procedure instead. |
| `E0306` | name is not declared | No local, parameter or constant with this name is in scope here. |
| `E0307` | '-' needs a signed integer or a float | Unsigned integers have no negation. Subtract from a value, or use a signed type. |
| `E0308` | '!' needs a bool | There is no truthiness. Compare instead: x == 0. |
| `E0309` | arithmetic needs two numbers of one type | + - * / need two integers or two floats of the same type; % needs integers. There are no implicit conversions: cast one side. |
| `E0310` | ordering needs two numbers of one type | < <= > >= compare two integers or two floats of the same type. |
| `E0311` | cannot compare these values | == and != need two values of one type. Structs, arrays and views are compared field by field or element by element, by you. |
| `E0312` | boolean operator needs two bools | && and \|\| take bools and short-circuit. |
| `E0313` | print needs Basic | print is in module Basic. Add #import "Basic"; at the top of the file. |
| `E0314` | not a procedure | A constant or variable was called like a procedure. |
| `E0315` | procedure is not declared | No procedure has this name. Procedures may be declared after they are used. |
| `E0316` | wrong number of arguments | The call passes a different number of arguments than the procedure declares. |
| `E0317` | argument has the wrong type | Each argument must have its parameter's type exactly. An array is passed as a view with name[..]. |
| `E0318` | print needs a format string | print("text\n"); the format comes first. |
| `E0319` | print's format must be a literal | The format is checked while compiling, so it must be a string literal. |
| `E0320` | format slots and values differ | Each % in the format takes one value. Write %% for a percent sign. |
| `E0321` | cannot print nothing | This call returns no value, so there is nothing to print. |
| `E0322` | value is not used | An expression statement must do something: call a procedure, or assign its value. |
| `E0323` | result is not used | The procedure returns a value that is dropped. Use it, or store it. |
| `E0324` | cannot initialize from nothing | The call returns no value, so it cannot initialize a variable. |
| `E0325` | initializer has the wrong type | The variable's declared type and its initializer differ. There are no implicit conversions. |
| `E0326` | compound assignment needs a number | += -= *= /= need a number target; %= &= \|= ^= <<= >>= need an integer. |
| `E0327` | assigned value has the wrong type | The value's type differs from the target's. |
| `E0328` | missing return value | The procedure declares a result, so return needs a value. |
| `E0329` | procedure returns nothing | A value is returned from a procedure without '-> T'. |
| `E0330` | returned value has the wrong type | The value differs from the declared result type. |
| `E0331` | condition must be bool | if, while and ifx take a bool. Compare explicitly: x != 0. |
| `E0332` | module imported twice | Remove the second #import. |
| `E0333` | unknown module | coax0 has one module, Basic. |
| `E0334` | procedure declared twice | There are no overloads yet; every procedure name is unique. |
| `E0335` | name is a builtin | print (with Basic) and type_name are reserved by the compiler. |
| `E0336` | no main | A program needs main :: () { }. A file whose procedures are #export_c can be built as a library with coax lib instead. |
| `E0337` | main has the wrong signature | main takes no parameters and returns nothing in coax0. |
| `E0338` | missing return | Some path reaches the end of a procedure that must return a value. |
| `E0340` | constant dependency cycle | Constants may refer to each other in any order, but not in a circle. |
| `E0341` | constant has no value | A constant or #run must produce a value; a call returning nothing cannot. |
| `E0342` | name is a constant, type or intrinsic | A local cannot take the name of a constant, struct, enum or intrinsic. |
| `E0343` | type_name needs one value | type_name(x) takes exactly one expression with a value. It is checked but never run. |
| `E0344` | cannot assign a constant or parameter | Constants never change. Parameters are immutable: copy one into a local to update it. |
| `E0345` | duplicate or conflicting constant | The constant's name is already a constant, procedure or intrinsic. |
| `E0346` | #assert needs bool | The asserted expression must be a bool, at the top level and in a statement. |
| `E0347` | constant dependencies too deep | Constant type resolution is bounded at depth 64. |
| `E0350` | value does not fit its type | Untyped literals take the type their use needs, if the value fits: x: u8 = 300 does not. A float literal cannot become an integer. Use a wider type, or cast,trunc to keep the low bits. |
| `E0351` | no such field or member | The struct has no field, or the enum no member, with this name. Arrays, views and strings have .count. |
| `E0352` | duplicate field or member | A field or member name appears twice in a declaration or a literal. |
| `E0353` | struct contains itself | A struct cannot hold itself by value; its size would be infinite. Hold it through a view, or by index. |
| `E0354` | a type is not a value | A struct or enum name was used as a value. Make a struct with Name.{ }, or name an enum member with Name.member. |
| `E0355` | cannot index or slice this | Only arrays, views and strings can be indexed and sliced. |
| `E0356` | index must be an integer | Indices and slice bounds are integers of any width. They are checked against the length at run time. |
| `E0357` | array length not known while checking | An array length is a positive integer known while checking: a literal, a constant, size_of, or arithmetic on them. Lengths computed by #run are planned. |
| `E0358` | array literal has the wrong count | [N] T.[...] gives exactly N elements. |
| `E0359` | cannot assign this | The target is a loop variable, a string byte, .count, or a temporary. Strings are immutable; copy the bytes into [N] u8 to change them. |
| `E0360` | break or continue outside a loop | break and continue belong inside while and for. |
| `E0361` | deferred code cannot leave its block | A deferred statement runs while its block exits, so it cannot return, break or continue out of it. |
| `E0362` | invalid cast | cast converts between numbers, and between enums and integers. cast,trunc converts between integer types only. |
| `E0363` | invalid enum value | Enum values are integers known while checking, fit the underlying type, and are distinct. |
| `E0364` | cannot loop over this | for takes a half-open range lo..<hi of one integer type, an array, a view, or a string. |
| `E0365` | ifx branches differ | Both branches of ifx produce a value of the same type. |
| `E0366` | a view needs a variable array | A view borrows an array's storage and may write through it, so the array must be a local variable. Copy a parameter or temporary into a local first. |
| `E0367` | field or element has the wrong type | The value given for a struct field, field default or array element has a different type. |
| `E0369` | invalid export | main cannot be exported, and exported names cannot start with coax_, which the generated runtime uses. |
| `E0370` | bit operation needs integers | & \| ^ need two integers of one type; << >> shift an integer by an integer amount from 0 to its width minus one. |
| `E0371` | field name is reserved in C | Struct fields keep their names in generated C and headers, so they cannot be C or C++ keywords or start with '_'. |
| `E0372` | type name conflicts | A struct or enum name is already used by another type, a procedure, a constant, or a built-in. |
| `E0373` | enum base must be an integer | An enum's underlying type is one of s8..s64 or u8..u64. |
| `E0374` | constant index out of bounds | An index written as a constant must be inside a fixed array's length. Indexes computed at run time are checked when they run. |
| `E0375` | math or string intrinsic misused | sin cos tan exp log sqrt floor abs take one float, pow two floats of one type; len takes a string, array or view; byte_at takes a string and an index. Convert integers first: f64(x). |
| `E0376` | procedure cannot cross the C boundary | A #foreign procedure passes only integers, floats and bool, and its name must be a valid C symbol that is not main and does not start with coax_. |
| `E0377` | outside the Metal scalar profile | coax metal accepts f32 and bool values, if/else, locals and acyclic calls. Loops, strings, integers, f64, print, #assert and #foreign calls stay in the C library. |
| `E0378` | byte intrinsic argument mismatch | bytes_overlap takes byte views or strings. Float bit conversions take their matching f32/u32 or f64/u64 type. |
| `E0379` | #forall domain cannot be enumerated | A #forall binding must be nonempty: bool, a nonempty enum, an integer of at most 16 bits, or an increasing lo..<hi range of int constants. Test a wider type on a small instance: write the code so a smaller type or range exercises the same paths. |
| `E0380` | procedure not exercised by a check | Every procedure except main and #foreign imports must actually execute during an #authority check. Dead branches and pre-folded #run results do not count. Execution coverage is not a proof of the assertions. |
| `E0381` | type checker resource limit | The Coax checker needs its caller-owned graph, type, local, diagnostic and text capacities and at most 256 active semantic entries. Reduce nesting or use CK_caps. Failed checking never admits a partial typed tree. |
| `E0382` | invalid reflection category | field_info takes a struct type and enum_info an enum type. The type must have a field or member to select. Use type_info for scalar, array or view layout. |
| `E0383` | returned borrow may outlive local storage | A returned view or string may borrow a local fixed array, directly or through a local alias, subrange, cast or ifx. Return the fixed array value or use caller-owned storage. This conservative local check does not establish general lifetime safety. |

## E04xx: compile-time evaluation

| Code | Meaning | What to do |
| --- | --- | --- |
| `E0400` | compile-time step budget exceeded | Compile-time evaluation is bounded at 100000 steps per compilation; each array element created costs one. Each #test and #forall has its own budget, 20000000 steps by default (coax test --steps N). Make the computation smaller or move it to run time. |
| `E0401` | compile-time code cannot capture a runtime value | #run code and constants see only constants and their own locals. |
| `E0402` | compile-time cycle | A constant, or a struct's field defaults, depend on themselves while being evaluated. |
| `E0403` | compile-time depth exceeded | Compile-time calls and dependencies are bounded at depth 64. |
| `E0404` | compile-time code cannot do I/O | print and other host effects are refused while compiling, so builds cannot depend on hidden effects. |
| `E0405` | compile-time read of an uninitialized variable | A variable declared with = --- was read before anything was assigned to it. |
| `E0406` | assertion failed | An #assert condition evaluated to false: at the top level while compiling, or as a statement inside a #test, a #forall or code they call. At run time the same statement traps in debug and release builds. |
| `E0407` | compile-time overflow or division by zero | Compile-time arithmetic is checked in every build mode, so release settings cannot change generated constants. |
| `E0408` | constant call needs #run | A constant that calls a procedure must say so: VALUE :: #run compute(); |
| `E0409` | compile-time result contains a view | A view points at memory that exists only while compiling, so it cannot be baked into the program. Return the array itself. |
| `E0410` | compile-time index out of bounds | An index or slice evaluated while compiling is outside its array, view or string. |
| `E0411` | compile-time cast or shift out of range | A cast evaluated while compiling does not fit its target, a value is not a member of the enum, or a shift amount is out of range. |
| `E0412` | compile-time math has no finite result | A math intrinsic evaluated while compiling gave infinity or NaN, such as sqrt(-1.0) or log(0.0). Compile-time results must be finite. |

## E05xx: proto imports

| Code | Meaning | What to do |
| --- | --- | --- |
| `E0501` | invalid proto import | Use #import_proto "profile.pb" as Prefix; with an exact FileDescriptorSet path relative to the source file. |
| `E0502` | proto descriptor rejected | The descriptor is malformed, outside importer limits or uses an unsupported shape. The importer reports the exact reason; no placeholder types are generated. |
| `E0503` | retired in 0.6.0 | Was: external proto importer unavailable. The compiled Coax descriptor parser, generator and wire runtime are linked into coax0. |
| `E0504` | proto imports are CPU-only | Generated binary codecs are CPU code and cannot be compiled as Metal shaders. |
| `E0600` | invalid source load | Use #load "part.coax"; at file scope with a nonempty, NUL-free path. Loads are literal source inputs, never compile-time effects. |
| `E0601` | source unit cannot be read | The resolved source must be a readable regular file. Relative paths are resolved beside the unit containing #load. |
| `E0602` | source load cycle | A source unit loaded a canonical path that is still active. Break the cycle; completed shared dependencies load only once. |
| `E0603` | source graph budget exceeded | One compilation admits at most 64 distinct source units, 32 active units, 16 MiB of source bytes and 512 loads per unit. Source I/O is bounded before allocation and while reading. |
| `E0604` | unbalanced source unit | In a composed program every physical source must balance its own parentheses, brackets and braces within depth 128. Declarations cannot cross a source boundary. |
