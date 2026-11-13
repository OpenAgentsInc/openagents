// Every error code coax0 can report, with what it means and what to do.
// `coax explain CODE` prints one; `coax explain --all` prints them all, and
// the book's Appendix B is generated from it. A code keeps its meaning
// forever; a retired code is listed and never reused. tests/compile_time.py
// checks that every code in the source has an entry here.
#include "coax.h"

namespace coax {

const ErrorInfo ERRORS[] = {
    // ---- E01xx: lexer ----
    {"E0101", "block comment is never closed", "A /* comment has no matching */ before the end of the file. Block comments nest, so every /* inside needs its own */."},
    {"E0102", "retired in 0.3.0", "Was: floating-point literals are not supported. Floats arrived in 0.3.0."},
    {"E0103", "number has no digits after its base prefix", "0x and 0b must be followed by at least one digit of that base."},
    {"E0104", "integer literal does not fit in 64 bits", "Integer literals go up to 18446744073709551615 (the u64 maximum). Where the literal is used, it must also fit that type."},
    {"E0105", "unknown escape in a string", "Strings support \\n \\t \\r \\0 \\\\ and \\\". Unicode and hex escapes are planned."},
    {"E0106", "string is never closed", "A string ends with a \" on the same line. Strings cannot span lines yet."},
    {"E0107", "non-ASCII character outside a string or comment", "Identifiers and operators are ASCII. UTF-8 text belongs in strings and comments."},
    {"E0108", "unexpected character", "The character starts no token in Coax."},
    {"E0109", "bad floating-point literal", "The literal rounds to infinity (the largest f64 is about 1.8e308, the largest f32 about 3.4e38), or it is followed by something other than an f32 or f64 suffix."},
    {"E0110", "invalid character literal", "A character literal is one ASCII character or escape in single quotes ('a', '\\n', '\\''). It is an integer literal holding that byte, so it fits u8."},
    {"E0111", "lexer source or storage limit", "Source must fit signed 32-bit byte offsets and one-based EOF locations. Native lexer buffers must meet the Coax capacity contract."},

    // ---- E02xx: parser ----
    {"E0201", "expected a token", "The parser needed a specific token here, usually ';', ')' or '}'. For a missing ';' the caret points right after the previous token."},
    {"E0202", "expected a type", "A type is a name, a fixed array '[N] T', a view '[] T', an optional 'T?', or an admitted parameterized type such as Result(T,E). Parentheses group a type before an optional suffix."},
    {"E0203", "comparisons cannot be chained", "a < b < c is rejected rather than given a surprising meaning. Write a < b && b < c."},
    {"E0204", "reserved word not supported yet", "The word is reserved for a planned feature, such as using or enum_flags. coax0 refuses it rather than guessing what it means."},
    {"E0205", "expected an expression", "A value was needed here: a literal, a name, a call, or a parenthesized expression."},
    {"E0206", "local declarations with :: are not supported", "Constants, procedures and types are declared at the top level in coax0."},
    {"E0207", "cannot assign to this expression", "Only a variable, a struct field or an element (a[i]) can be assigned."},
    {"E0208", "expected a name", "A parameter, field, member or loop variable name was expected here."},
    {"E0209", "expected a module name after #import", "Write #import \"Basic\";"},
    {"E0210", "directive not supported here", "The directive is unknown or not implemented in this position. Procedures accept #export_c, #foreign and #inline; #inline is refused with either of the other two and on main and recursive procedures. Files accept #import and #assert; expressions accept #run."},
    {"E0211", "retired in 0.2.0", "Was: only procedures can be declared at the top level. Top-level constants arrived in 0.2.0."},
    {"E0212", "global variables are not supported", "Top-level mutable state is not part of coax0. Use constants, or pass state explicitly."},
    {"E0213", "expected a top-level declaration", "The top level holds #import, #assert and declarations 'name :: ...'."},
    {"E0214", "unknown cast modifier", "The only modifier is trunc: cast,trunc(u8) x keeps the low bits instead of checking that the value fits."},
    {"E0215", "struct literal needs a struct name", "Write the struct's name before .{ }: Point.{x = 1, y = 2}. Inferred literals are planned."},
    {"E0216", "ranges are half-open", "Coax ranges are written lo..<hi and include lo but not hi. '..' alone is not a range."},
    {"E0217", "array literal needs a fixed array type", "Write the type first: [3] int.[1, 2, 3]."},
    {"E0218", "a range loop has one variable", "for i: 0..<n { } has a counter only. Index variables belong to loops over arrays, views and strings."},
    {"E0219", "enum needs an underlying type", "An enum names its integer width: Outcome :: enum u8 { ... }. The width is part of its layout."},
    {"E0220", "malformed check", "#test takes a name string and a block: #test \"refunds never exceed payment\" { ... }. #forall takes one or more bindings, each a type or a lo..<hi range, then a block: #forall a: u8, b: 0..<10 { ... }. #steps N before the block gives the check its own budget of N steps, a positive integer literal: #test \"the world builds\" #steps 150000000 { ... }."},
    {"E0221", "parser resource limit", "The Coax parser requires an EOF-terminated lexer token stream, caller-owned indexed-tree storage and at most 128 active recursive grammar entries. Reduce syntax nesting or supply the capacities returned by PS_caps. No partial syntax tree is admitted after a failure. A build above 96 entries gets a near-limit note."},
    {"E0223", "malformed struct parameter list", "A generic struct lists its parameters as $NAME, each once, separated by ',': Pair :: struct($T) { a: $T; b: $T; } or Box :: struct($T, $N) { items: [$N] $T; }."},
    {"E0222", "qualified name reaches through an alias", "A qualified name is one module alias and one declaration: hud.Slot, hud.frame(x). Aliases are not reexported, so a.b.Name is refused; load that module with its own alias."},
    {"E0224", "malformed #run_steps", "#run_steps N; declares the compile-time step budget of the whole compilation, N a whole number from 1 through 250000000: #run_steps 250000000;. A file declares it at most once, and only the entry file's applies: a loaded file's is ignored, so a library declares what it needs when checked alone and never raises the budget of a program that loads it. --run-steps N on the command line overrides it."},
    {"E0225", "a spelling from another language", "The code uses a spelling from C, C++, Rust, Go, JavaScript, Python or Jai that Coax writes another way, and the message names the Coax spelling. Locals are declared with x := 1; or x: int = 1; (no let, var or const), procedures with add :: (a: int) -> int { }, types with Point :: struct { x: int; }, constants with N :: 3;. Casts are cast(T) x, the conditional expression is ifx c then a else b, loops are for i: 0..<n { } and while c { }, and x += 1 replaces x++. There are no pointers: pass an array as a view with name[..]. Integers are s8..s64 and u8..u64 (int is s64), floats f32 and f64, booleans true and false, an empty optional null. print(\"%\\n\", x) prints with #import \"Basic\";, and #assert c; asserts. coax vocab WORD prints the Coax spelling of a word from another language."},

    // ---- E03xx: checker ----
    {"E0301", "type not supported yet", "The type is in the plan (spec 6.2) but coax0 does not implement it, for example dynamic arrays '[..] T' or vectors."},
    {"E0302", "unknown type", "No built-in type, struct or enum has this name."},
    {"E0303", "name already declared; no shadowing", "Every local name in a procedure is unique among the names in scope. Coax has no shadowing, so a name always means one thing."},
    {"E0304", "retired in 0.52.0", "Was: name is already a procedure. A local, parameter or loop variable may share a procedure's name: procedures are not values, so name(...) calls the procedure and a bare name is the local."},
    {"E0305", "procedures are not values", "Procedure values, callbacks and procedure types such as (int) -> int are planned. Call the procedure instead: a bare name is a local, parameter or constant, never a procedure. To pick a procedure at run time, pass an enum that names the choice and branch on it where the call happens: Op :: enum u8 { add; mul; } apply :: (op: Op, a: int, b: int) -> int { if op == Op.add { return a + b; } return a * b; }."},
    {"E0306", "name is not declared", "No local, parameter or constant with this name is in scope here."},
    {"E0307", "'-' needs a signed integer or a float", "Unsigned integers have no negation. Subtract from a value, or use a signed type."},
    {"E0308", "'!' needs a bool", "There is no truthiness. Compare instead: x == 0."},
    {"E0309", "arithmetic needs two numbers of one type", "+ - * / need two integers or two floats of the same type; % needs integers. There are no implicit conversions, so f32 and f64 (or s32 and int) do not mix: cast one side. The fix line names the cast, such as cast(f32) b; when the result's type is known, it keeps the side that already has it. An untyped literal takes the other side's type by itself."},
    {"E0310", "ordering needs two numbers of one type", "< <= > >= compare two integers or two floats of the same type."},
    {"E0311", "cannot compare these values", "== and != need two values of one type. Structs, arrays and views are compared field by field or element by element, by you."},
    {"E0312", "boolean operator needs two bools", "&& and || take bools and short-circuit."},
    {"E0313", "retired in 0.52.0", "Was: print needs Basic. Basic is imported by default, so print needs no #import; #import \"Basic\"; is still accepted."},
    {"E0314", "not a procedure", "A constant or variable was called like a procedure."},
    {"E0315", "procedure is not declared", "No procedure has this name. Procedures may be declared after they are used."},
    {"E0316", "wrong number of arguments", "The call passes a different number of arguments than the procedure declares."},
    {"E0317", "argument has the wrong type", "Each argument must have its parameter's type exactly. An array is passed as a view with name[..]."},
    {"E0318", "print needs a format string", "print(\"text\\n\"); the format comes first."},
    {"E0319", "print's format must be a literal", "The format is checked while compiling, so it must be a string literal."},
    {"E0320", "format slots and values differ", "Each % in the format takes one value. Write %% for a percent sign."},
    {"E0321", "cannot print nothing", "This call returns no value, so there is nothing to print."},
    {"E0322", "value is not used", "An expression statement must do something: call a procedure, or assign its value. To evaluate an expression and drop its value on purpose, write _ = expression;."},
    {"E0323", "result is not used", "The procedure returns a value that is dropped. Use it, store it, or drop it on purpose with _ = f(...);. '_' names nothing: _ = e; and _ := e; evaluate e and drop the value, as often as you like. A Result is never dropped this way; handle it."},
    {"E0324", "cannot initialize from nothing", "The call returns no value, so it cannot initialize a variable."},
    {"E0325", "initializer has the wrong type", "The variable's declared type and its initializer differ. There are no implicit conversions."},
    {"E0326", "compound assignment needs a number", "+= -= *= /= need a number target; %= &= |= ^= <<= >>= need an integer."},
    {"E0327", "assigned value has the wrong type", "The value's type differs from the target's."},
    {"E0328", "missing return value", "The procedure declares a result, so return needs a value."},
    {"E0329", "procedure returns nothing", "A value is returned from a procedure without '-> T'."},
    {"E0330", "returned value has the wrong type", "The value differs from the declared result type."},
    {"E0331", "condition must be bool", "if, while and ifx take a bool. Compare explicitly: x != 0."},
    {"E0332", "module imported twice", "Remove the second #import."},
    {"E0333", "unknown module", "coax0 has one module, Basic."},
    {"E0334", "procedure declared twice", "There are no overloads yet; every procedure name is unique."},
    {"E0335", "name is a builtin", "print (with Basic) and type_name are reserved by the compiler."},
    {"E0336", "no main", "coax run and coax build make a program, which needs main :: () { }. A file with no main is a library: coax test, coax check, coax mutate and coax c take it as one without --library, and coax lib builds it."},
    {"E0337", "main has the wrong signature", "main takes no parameters and returns nothing in coax0."},
    {"E0338", "missing return", "Some path reaches the end of a procedure that must return a value."},
    {"E0339", "too many generic instances", "A program may make 256 generic instances, or one per 8 syntax nodes in a program of more than 2056: one per generic procedure or struct and distinct binding of its parameters. The message names the limit and lists the generics with the most instances, at their declarations; a build above three quarters of it gets a near-limit note with the same list. A chain of generic uses that binds ever larger types never ends; break it with a concrete type."},
    {"E0340", "constant dependency cycle", "Constants may refer to each other in any order, but not in a circle."},
    {"E0341", "constant has no value", "A constant or #run must produce a value; a call returning nothing cannot."},
    {"E0342", "name is a constant, type or intrinsic", "A local cannot take the name of a constant, struct, enum or intrinsic."},
    {"E0343", "type_name needs one value", "type_name(x) takes exactly one expression with a value. It is checked but never run."},
    {"E0344", "cannot assign a constant or parameter", "Constants never change. Parameters are immutable: copy one into a local to update it."},
    {"E0345", "duplicate or conflicting constant", "The constant's name is already a constant, procedure or intrinsic."},
    {"E0346", "#assert needs bool", "The asserted expression must be a bool, at the top level and in a statement."},
    {"E0347", "constant dependencies too deep", "Constant type resolution is bounded at depth 64."},
    {"E0348", "type parameter bound twice", "The arguments of one call bind each $T once: the first typed argument whose type uses $T binds it, and every other one must have that same type; an untyped literal for a bare $T takes the bound type. Pass the same type, or convert one with cast."},
    {"E0349", "argument does not have the parameter's shape", "A parameter written [] $T binds $T from a view, so its argument must be a view (pass name[..] for an array); one written [$N] T or [4] T binds or checks a fixed array's length, so its argument must be a fixed array of that length."},
    {"E0350", "value does not fit its type", "Untyped literals take the type their use needs, if the value fits: x: u8 = 300 does not. A float literal cannot become an integer. Use a wider type, or cast,trunc to keep the low bits."},
    {"E0351", "no such field or member", "The struct has no field, or the enum no member, with this name. Arrays, views and strings have .count."},
    {"E0352", "duplicate field or member", "A field or member name appears twice in a declaration or a literal."},
    {"E0353", "struct contains itself", "A struct cannot hold itself by value; its size would be infinite. Hold it through a view, or by index."},
    {"E0354", "a type is not a value", "A struct or enum name was used as a value. Make a struct with Name.{ }, or name an enum member with Name.member."},
    {"E0355", "cannot index or slice this", "Only arrays, views and strings can be indexed and sliced."},
    {"E0356", "index must be an integer", "Indices and slice bounds are integers of any width. They are checked against the length at run time."},
    {"E0357", "array length not known while checking", "An array length is a positive integer known while checking: a literal, a constant, size_of, or arithmetic on them. Lengths computed by #run are planned."},
    {"E0358", "array literal has the wrong count", "[N] T.[...] gives exactly N elements."},
    {"E0359", "cannot assign this", "The target is a loop variable, a string byte, .count, or a temporary. Strings are immutable; copy the bytes into [N] u8 to change them."},
    {"E0360", "break or continue outside a loop", "break and continue belong inside while and for."},
    {"E0361", "deferred code cannot leave its block", "A deferred statement runs while its block exits, so it cannot return, break or continue out of it."},
    {"E0362", "invalid cast", "cast converts between numbers, and between enums and integers. cast,trunc converts between integer types only."},
    {"E0363", "invalid enum value", "Enum values are integers known while checking, fit the underlying type, and are distinct."},
    {"E0364", "cannot loop over this", "for takes a half-open range lo..<hi of one integer type, an array, a view, or a string."},
    {"E0365", "ifx branches differ", "Both branches of ifx produce a value of the same type."},
    {"E0366", "a view needs a variable array", "A view [] T borrows an array's storage and may write through it, so the array must be a local variable. To only read a constant or a parameter, borrow it as a read-only view: type the receiving parameter or variable [] const T, as in sum :: (v: [] const int) -> int and sum(K[..]). To write, copy it into a local first. A temporary (a call's result or an array literal) has no storage to borrow: put it in a local, then take its view."},
    {"E0367", "field or element has the wrong type", "The value given for a struct field, field default or array element has a different type."},
    {"E0368", "type and value parameters share a name", "A $NAME is either a type parameter ($T, written as a type) or a value parameter ($N, written as a fixed array length [$N] T) in one procedure, not both. Give the length its own name."},
    {"E0369", "invalid export", "main cannot be exported, and exported names cannot start with coax_, which the generated runtime uses."},
    {"E0370", "bit operation needs integers", "& | ^ need two integers of one type; << >> shift an integer by an integer amount from 0 to its width minus one."},
    {"E0371", "field name is reserved in C", "Struct fields keep their names in generated C and headers, so they cannot be C or C++ keywords or start with '_'."},
    {"E0372", "type name conflicts", "A struct or enum name is already used by another type, a procedure, a constant, or a built-in."},
    {"E0373", "enum base must be an integer", "An enum's underlying type is one of s8..s64 or u8..u64."},
    {"E0374", "constant index out of bounds", "An index written as a constant must be inside a fixed array's length. Indexes computed at run time are checked when they run."},
    {"E0375", "math or string intrinsic misused", "sin cos tan exp log sqrt floor abs take one float, pow two floats of one type; len takes a string, array or view; byte_at takes a string and an index. Convert integers first: f64(x)."},
    {"E0376", "procedure cannot cross the C boundary", "A #foreign procedure passes integers, floats, bool and borrowed views of them, returns a scalar or nothing, and its name must be a valid C symbol that is not main and does not start with coax_."},
    {"E0377", "outside the Metal scalar profile", "coax metal accepts f32 and bool values, if/else, locals and acyclic calls. Loops, strings, integers, f64, print, #assert and #foreign calls stay in the C library."},

    {"E0378", "byte intrinsic argument mismatch", "bytes_overlap takes byte views or strings. Float bit conversions take their matching f32/u32 or f64/u64 type. add_wrap, sub_wrap, mul_wrap, add_checked, sub_checked, mul_checked, add_sat, sub_sat and mul_sat take two integers of one type; rotl and rotr two u32 or two u64."},
    {"E0379", "#forall domain cannot be enumerated", "A #forall binding must be nonempty: bool, a nonempty enum, an integer of at most 16 bits, or an increasing lo..<hi range of int constants. Test a wider type on a small instance: write the code so a smaller type or range exercises the same paths."},
    {"E0380", "procedure not exercised by a check", "Every procedure except main and #foreign imports must actually execute during an #authority check. Dead branches and pre-folded #run results do not count. Execution coverage is not a proof of the assertions."},

    {"E0381", "type checker resource limit", "The Coax checker needs its caller-owned graph, type, local, diagnostic and text capacities and at most 256 active semantic entries. Reduce nesting or use CK_caps. When generic instances fill the node store (16 times the program's own nodes), the error lists the generics whose instances copied the most nodes. Failed checking never admits a partial typed tree. A build above 192 entries gets a near-limit note. Borrow analysis gives each declaration 2097152 steps in each pass and names the one that runs out; split it into smaller procedures. A declaration above three quarters of that gets a near-limit note."},

    {"E0382", "invalid reflection category", "field_info takes a struct type and enum_info an enum type. The type must have a field or member to select. Use type_info for scalar, array or view layout."},

    {"E0383", "borrow may outlive its backing storage", "A returned descriptor or retained store can carry local backing beyond its frame, including through helpers, aggregates and active sum payloads. Return an owned value or use caller-owned backing. The bounded A1/B1 checks retain known local witnesses even with incomplete alternatives; general region lifetimes, B2 local-carrier composition and B3 request ownership remain separate work."},
    {"E0384", "unknown module alias", "A qualified name's alias must be bound in this module by #load \"FILE.coax\" as NAME;. Aliases belong to the logical module that loads them."},
    {"E0385", "module has no such declaration", "A qualified name sees only the declarations of the alias's module. Check the spelling, or load the module that declares it."},
    {"E0386", "ambiguous unqualified name", "Two modules declare this name and the use's own module does not. Qualify it with the alias of the module you mean: alias.Name. A file with no #module is in the unowned program. When it loads a module's file with no alias and uses a name that module offers (the module has no #export list, or exports the name) while another file with no #module declares the name too, the use is ambiguous: it would mean the other file's in silence. Rename one, load the module as an alias and qualify the name, or keep the module's behind an #export list."},
    {"E0389", "private to its module", "A module with an #export list keeps every other declaration private. Use an exported name, or add the declaration to its module's #export list."},
    {"E0390", "export of an undeclared name", "An #export list names declarations of its own logical module. Reexports of another module's names are not supported."},
    {"E0391", "name exported twice", "List each exported name once across a module's #export lists."},
    {"E0392", "type parameter appears only in the result", "A $T in a procedure's result type must first appear in one of its parameter types, which binds it at each call. Add a parameter whose type uses $T, or return a concrete type."},
    {"E0393", "type parameter is not bound here", "Only a procedure's parameter types bind a type parameter. A $T first written in a procedure body, or in a constant, check or other declaration outside a procedure signature, has nothing to bind it."},
    {"E0394", "nested generic type", "A placeholder is accepted as $T, as a view [] $T and as a fixed array [N] $T or [$N] $T. A view or array of a composite that holds a placeholder, such as [] [] $T, [] [$N] T or [4] [] $T, is not supported yet."},
    {"E0395", "type parameter in a struct field", "A struct field may use only the parameters its struct lists: Pair :: struct($T) { a: $T; b: $T; }. A plain struct lists none, so its fields have concrete types."},
    {"E0396", "recursion through a generic procedure", "A generic procedure cannot call itself: each instance would need a bounded specialization budget, which is not implemented. Write the recursion in a concrete procedure."},
    {"E0397", "argument has no value to bind a type parameter", "A call's argument binds $T to its type, and a call that returns nothing has none. Pass a value of the type the procedure should work on."},
    {"E0398", "generic procedure cannot be exported", "#export_c gives a procedure one C name and signature, which a procedure with type parameters does not have. Export a concrete procedure."},
    {"E0399", "query depends on the instance", "size_of, type_info, field_info, enum_info and type_name of a type parameter (or a view of one) depend on the type each call binds, and generic instantiation is not supported yet."},
    {"E0387", "C cannot spell a module collision", "Two modules may each declare a name, and their private C spellings are qualified. An #export_c or #foreign symbol, or a type in a C library header, is one global C name, so a collision there is refused, never merged. Rename one."},
    {"E0388", "alias is not reexported", "alias.Name cannot name another module's alias. Load the module you mean here with its own alias."},

    {"E0820", "invalid unit annotation", "@unit takes all six literal metadata fields and annotates an empty nongeneric tag. Exponents are -8..8 and positive rational factors are 1..1048576. An annotation is checked metadata, not runtime storage."},
    {"E0821", "invalid quantity family or carrier", "@builtin_quantity requires two parameters and a single value field of the second numeric parameter, with no default. Concrete units must carry @unit metadata; the initial carrier profile is f32 or f64."},
    {"E0822", "quantity units differ", "Addition, subtraction and comparison require the same nominal unit. Write an explicit supported convert_unit(TargetUnit, value); equal dimensions do not imply an implicit conversion."},
    {"E0823", "quantity representations differ", "Quantity arithmetic and scalar scaling use one f32 or f64 carrier. Convert the raw representation explicitly before constructing a different quantity carrier."},
    {"E0824", "quantity operator is outside the initial profile", "The initial profile admits matching quantity addition/subtraction/comparison and dimensionless scalar scaling. Quantity products and quotients use checked derived units. Raw scalar divided by quantity, compound updates and affine/frame operators remain pending."},
    {"E0825", "invalid explicit quantity conversion", "convert_unit takes a concrete declared target unit and one quantity with matching mass/length/time/angle dimensions. Initial conversions require concrete unit metadata."},
    {"E0826", "unit ratio needs a later rounding profile", "The initial conversion profile admits only an exact power-of-two scale ratio. Normal representable results are exact; subnormal rounding and overflow follow carrier IEEE arithmetic. Other unit ratios are refused."},
    {"E0827", "quantity reflection is not in schema1", "Transparent quantities erase to scalar C carriers. Existing schema1 type_info/field_info records do not describe that semantic-to-physical relation; use size_of and type_name until versioned metadata is qualified."},

    {"E0828", "quantity device profile is unqualified", "This initial quantity slice qualifies transparent scalar carriers on the C backend only. Metal, CUDA, SPIR-V and WGSL quantity output is explicitly refused until its own representation/ABI evidence exists."},
    {"E0829", "derived physical metadata is outside the profile", "Each intermediate unit must have mass/length/time/angle exponents -8..8 and reduced rational scale factors 1..1048576. Exact cancellation happens before the scale bound. Convert operands explicitly when needed."},
    {"E0830", "invalid derived unit type expression", "UnitOne takes no arguments; UnitProduct and UnitQuotient take two concrete unit types. These reserved forms are types, not runtime calls. Symbolic generic unit operands are a later profile."},
    {"E0831", "derived nominal unit expression is too complex", "Canonical forms contain at most eight nominal atoms with nonzero powers from -2147483647 through 2147483647. Matching atoms cancel before the capacity check; no partial unit is admitted."},
    {"E0832", "invalid frame metadata or declaration", "Declare one empty nongeneric frame tag with explicit handedness, convention, continuity and minimum precision. Optical is right-handed X-right/Y-down/Z-forward."},
    {"E0833", "invalid spatial type", "Point3 and Vector3 require an admitted concrete frame, unit and f32/f64; points require length units and all coordinates meet the frame precision floor."},
    {"E0834", "spatial operation refused", "Use the admitted affine point/vector operations with identical frame, unit and carrier. Only matching raw f32/f64 values scale vectors; transforms and implicit conversions are not provided."},
    {"E0835", "spatial reflection profile unavailable", "Schema1 does not describe spatial semantics. Use size_of, physical layout or type_name until versioned frame metadata is qualified."},
    {"E0836", "spatial device profile unavailable", "The initial Point3/Vector3 representation and operator profile is qualified only for native C."},
    {"E0840", "plain arithmetic on a counter", "A struct field declared #counter changes only through the named integer operations, which do the same in every build: add_checked, sub_checked and mul_checked return none when the result does not fit, add_sat, sub_sat and mul_sat stop at the type's bound, add_wrap, sub_wrap and mul_wrap wrap. Plain +, -, * and their compound assignments on it are refused; /, % and comparisons are not."},

    // ---- E04xx: compile-time evaluation ----
    {"E0400", "compile-time resource budget exceeded", "Compile-time evaluation defaults to 100000 shared steps; #run_steps N; in the entry file or --run-steps N (which overrides it) selects a positive limit up to 250000000. Aggregate storage has an independent fixed cap (1572864 value slots, 131072 arrays and structs; coax test and mutate allow 33554432 and 2097152), and the error names which limit, what was needed and the limit. Each #test and #forall has its own budget, 20000000 steps by default (coax test --steps N). Aggregate traversal is 134217728 value and field visits per compilation or check; --traversal N, or #traversal N on a check, selects up to 68719476736. The error names the resource, what was needed and the limit, its top five users with their locations (procedures, call sites that copy arguments, constants and checks; the arrays and structs holding value slots) and one reduction, also as a capacity object in both JSON formats. A compilation that uses more than three quarters of a limit (--near-limit P or COAX_NEAR_LIMIT selects another percent) gets a near-limit note with the same breakdown."},
    {"E0401", "compile-time code cannot capture a runtime value", "#run code and constants see only constants and their own locals."},
    {"E0402", "compile-time cycle", "A constant, or a struct's field defaults, depend on themselves while being evaluated."},
    {"E0403", "compile-time depth exceeded", "Compile-time calls and dependencies are bounded at depth 64. A compilation above depth 48 gets a near-limit note."},
    {"E0404", "compile-time code cannot do I/O", "print and other host effects are refused while compiling, so builds cannot depend on hidden effects."},
    {"E0405", "compile-time read of an uninitialized variable", "A variable declared with = --- was read before anything was assigned to it."},
    {"E0406", "assertion failed", "An #assert condition evaluated to false: at the top level while compiling, or as a statement inside a #test, a #forall or code they call. At run time the same statement traps in debug and release builds."},
    {"E0407", "compile-time overflow or division by zero", "Compile-time arithmetic is checked in every build mode, so release settings cannot change generated constants."},
    {"E0408", "constant call needs #run", "A constant that calls a procedure must say so: VALUE :: #run compute();"},
    {"E0409", "compile-time result contains a view", "A view points at memory that exists only while compiling, so it cannot be baked into the program. Return the array itself."},
    {"E0410", "compile-time index out of bounds", "An index or slice evaluated while compiling is outside its array, view or string."},
    {"E0411", "compile-time cast or shift out of range", "A cast evaluated while compiling does not fit its target, a value is not a member of the enum, or a shift amount is out of range."},
    {"E0412", "compile-time math has no finite result", "A math intrinsic evaluated while compiling gave infinity or NaN, such as sqrt(-1.0) or log(0.0). Compile-time results must be finite."},
    {"E0413", "native check emission is not qualified", "The verification-only checks-c command supports selected zero-argument #test bodies and their admitted checked callees. Selected #forall bodies, host effects, product main and unsupported representation/lifetime forms refuse at their original source location. This does not change ordinary product emission or waive authority checks."},
    {"E0414", "mutant survived", "coax mutate changed this code and every check still passed, so no check depends on what it does. Add a check that fails for the change, or end the line with // mutant: equivalent when the change can't alter behavior. Printed by coax mutate --diagnostics json."},
    {"E0415", "procedure not reached by any check", "coax mutate found no check that calls this procedure, so none of its mutants can be killed. Call it from a #test or #forall. Printed by coax mutate --diagnostics json."},
    // ---- E05xx: proto imports ----
    {"E0501", "invalid proto import", "Use #import_proto \"profile.pb\" as Prefix; with an exact FileDescriptorSet path relative to the source file."},
    {"E0502", "proto descriptor rejected", "The descriptor is malformed, outside importer limits or uses an unsupported shape. The importer reports the exact reason; no placeholder types are generated."},
    {"E0503", "retired in 0.6.0", "Was: external proto importer unavailable. The compiled Coax descriptor parser, generator and wire runtime are linked into coax0."},
    {"E0504", "proto imports are CPU-only", "Generated binary codecs are CPU code and cannot be compiled as Metal shaders."},
    // ---- E06xx: source units ----
    {"E0600", "invalid source load", "Use #load \"part.coax\"; at file scope with a nonempty, NUL-free path, #load \"part.coax\" as name; to bind its module to an alias, or #load \"part-checks.coax\" #test; to load a unit's checks. Loads are literal source inputs, never compile-time effects."},
    {"E0601", "source unit cannot be read", "The resolved source must be a readable regular file. Relative paths are resolved beside the unit containing #load."},
    {"E0602", "source load cycle", "A source unit loaded a canonical path that is still active. Break the cycle; completed shared dependencies load only once."},
    {"E0603", "source graph budget exceeded", "One compilation admits at most 4096 distinct source units, 32 active units, 64 MiB of source bytes and 512 loads per unit. Source I/O is bounded before allocation and while reading. A build above three quarters of any of these gets a near-limit note."},
    {"E0604", "unbalanced source unit", "In a composed program every physical source must balance its own parentheses, brackets and braces within depth 128. Declarations cannot cross a source boundary."},
    {"E0605", "invalid logical module", "Write #module \"engine.hud\"; once per source unit, at file scope. An ID is 1 to 128 bytes of lowercase dotted segments; each segment starts with a-z and continues with a-z, 0-9 or _."},
    {"E0606", "logical module claimed twice", "Two different canonical source units claimed one logical module ID. A module has one authored owner unit; its other files are #load members of it."},
    {"E0607", "source unit under two logical modules", "A unit without its own #module is a member of the modules whose units load it, and it may belong to only one. Give the shared unit its own #module, or load it under one module."},
    {"E0608", "module parameters are not supported", "An aliased #load takes the module as written: #load \"hud.coax\" as hud;. Configured or parameterized modules are not part of coax0."},
    {"E0609", "package search is not supported", "An aliased #load names a .coax file by its path, relative to the loading unit. Nothing searches packages or installs dependencies."},
    {"E0610", "alias target has no logical module", "An alias binds to a logical module ID, so the loaded unit must claim one with #module \"some.id\";. Load a member file without an alias."},
    {"E0611", "alias names two modules", "Within one logical module an alias names one module. Pick another alias for the second module."},
    {"E0612", "unsupported layout target", "Layout facts are qualified only for the named native-lp64 target. coax layout and coax deps refuse any other --target instead of reporting LP64 values under its name."},
    {"E0613", "malformed export list", "#export takes file-scope identifiers separated by ',' and ends with ';': #export hud_slot, HudRect; A unit exports at most 512 names."},
    {"E0614", "reexport in an export list", "An export list names only its module's own declarations; alias.Name is a reexport, which is not supported."},
    {"E0615", "export outside a logical module", "#export belongs to a unit under a logical module. Claim one with #module, or load the file from a unit that does."},
    {"E0616", "emitted code calls a check unit", "A unit reached only through #load \"FILE.coax\" #test; is a check unit: its procedures are checked and counted for coverage and never emitted, and neither is a procedure that calls one. main and an #export_c procedure are always emitted, so they may not reach one. Load the file without #test, or move the procedure."},
    // ---- E07xx: device code ----
    {"E0700", "outside the CUDA compute subset", "coax cuda accepts bool, f32, f64 and sized integers, fixed arrays of at most 4096 scalars, views of numbers, if/else, for loops, break, continue and acyclic calls. Strings, structs, enums, while loops, defer, #assert, print and #foreign calls stay in the C library. Every #export_c procedure is a kernel: it returns nothing and its first parameter is its int work item. A kernel with for thread: 0..<T loops (T a multiple of 32 up to 1024) runs T threads per item: a thread loop writes a shared array only at the elements its thread owns, code between thread loops never writes one, and shared arrays fit in 32 KiB."},
    {"E0701", "CUDA module has no kernel", "coax cuda compiles the #export_c procedures of a file as kernels, with what they call. Mark at least one procedure #export_c."},
    {"E0702", "outside the Metal compute subset", "coax metal-compute accepts what coax cuda accepts (E0700). Apple GPUs have no binary64, so f64 is a float-float pair: f64_bits, f64_from_bits, sin, cos, tan and pow of an f64 and conversion of an f64 to an integer are refused. A view points into device memory, so a slice of a kernel's fixed array is refused except as a whole argument to a device primitive. The Apple matrix primitives (am_sg_mma_64x32x32, am_sg_spill_64x32, am_matmul_bf16) belong to group kernels of 128 threads (64, 128 or 256 for am_matmul_bf16), called between thread loops."},
    {"E0703", "Metal compute module has no kernel", "coax metal-compute compiles the #export_c procedures of a file as kernels, with what they call. Mark at least one procedure #export_c."},
    {"E0704", "outside the SPIR-V lowering", "coax spirv lowers what coax metal accepts (f32 and bool, if/else and acyclic calls) to SPIR-V 1.3 for Vulkan. --kernel NAME wraps one procedure of f32 parameters, returning f32 or bool, in a GLCompute entry point; without it the module is a library. A module has at most 256 procedures, 1024 locals in one procedure, 1024 constants and 32 arguments in one call."},
    {"E0705", "shader entry point not found", "coax spirv --vertex NAME and --fragment NAME make entry points of procedures declared in the file or the units it loads. Name one of them, spelled as declared."},
    {"E0706", "outside the WGSL writer", "coax wgsl writes a whole module the way lib/compiler/wgsl.coax does. The construct named in the message is one that writer doesn't accept; rewrite it with what coax spirv lowers, or keep it in the C library."},
    {"E0707", "outside the Metal graphics writer", "coax metal --vertex/--fragment writes the checked graphics subset in lib/compiler/metal-shader.coax. Keep portable storage layouts, stage interfaces and the reported resource slot limits."},

    // ---- E08xx: generic structs ----
    {"E0800", "generic struct without arguments", "A type that names a generic struct gives an argument for each parameter: Pair(int), Box(f64, 4). A literal Pair.{a = 1, b = 2} deduces them from its fields instead."},
    {"E0801", "wrong number of struct arguments", "A generic struct takes one argument for each parameter it lists: Box :: struct($T, $N) is written Box(int, 4)."},
    {"E0802", "generic struct over a borrow", "A generic struct holds fixed storage only. Binding a parameter to a view, or to a struct or array that holds one, would carry a borrow past the rule that a view does not outlive its array. Store the array itself, or an index into it."},
    {"E0803", "arguments on a type that takes none", "Type arguments require a generic struct or a compiler-owned parameterized family such as Result(T,E). Other builtin types, enums and structs without parameters are written by name alone."},
    {"E0804", "struct parameter not deduced", "A literal of a generic struct deduces each parameter from a field given in it. Give a field that uses the parameter, or declare the variable's type: x: Pair(int) = Pair.{...}."},
    {"E0805", "unknown constraint", "A type parameter's constraint is one of equatable (== and !=), ordered (also < <= > >=), numeric (also + - * / and integer literals) or copyable (assignment, which every coax0 type allows). A length parameter $N takes no constraint."},
    {"E0806", "binding outside a constraint", "A call or instance gives a constrained parameter a type that does not meet its constraint. The error is at the use and names the generic declaration and the constraint. Ordered and numeric types are integers and floats; equatable types are scalars, strings and enums."},
    {"E0807", "instantiation failed", "A generic body checked against its constraints can still fail for one instance, for example a literal too large for u8. The instance's errors come first; this one is at the call that made the instance and lists its bindings."},
    {"E0808", "misplaced constraint", "A constraint is written once, where the parameter is bound: on its first $T in a procedure's parameters, or in a struct's parameter list. Elsewhere, and a second different constraint on one parameter, are refused."},
    // ---- E09xx: tagged results and effect admission ----
    {"E0900", "invalid tagged type or Result containment", "A nominal union has1 to64 unique alternatives. Result(T,E) has explicit success and non-void error types; a Result cannot be hidden transitively in another Result, optional, array, view, record or union in the initial handling profile."},
    {"E0901", "invalid active alternative constructor", "Construct exactly one declared alternative with its checked payload, or a unit alternative with no payload. Result constructors spell both type arguments."},
    {"E0902", "tagged value needs valid construction", "Union and Result values have no implicit default. Optional defaults to none. Explicit =--- is refused for all sums and by-value arrays or records containing them; supply a valid constructor."},
    {"E0903", "tagged payload needs elimination", "A sum's payload cannot be read as an ordinary field. Use an exhaustive switch, try, or or_else so only the active alternative is accessed."},
    {"E0904", "invalid or nonexhaustive switch", "Switch a union, Result, optional or enum and cover every alternative exactly once. Payload alternatives bind one immutable local; unit and enum cases bind none. There is no default, fallthrough or guarded case in this profile."},
    {"E0905", "propagation family or error type differs", "try requires Result(T,E) inside a procedure returning Result(U,E), with the exact same nominal E, or T? inside a procedure returning U?. Map other families explicitly with switch."},
    {"E0906", "fallback has the wrong payload type", "The lazy right side of or_else must have the success payload type. A unit success needs a void fallback expression."},
    {"E0907", "Result would leave without handling", "Naming or assigning an error does not handle it. Consume or transfer each direct Result before every scope exit, including an implicit error return from try and a later failing call argument."},
    {"E0908", "Result has already transferred or cannot be reassigned", "Direct Result bindings are immutable handling obligations. Transfer into a new local, direct Result parameter or return only once; the old binding cannot be read again."},
    {"E0909", "Result handling paths do not agree", "Continuing branches must agree on pending and consumed Results. An outer Result cannot be consumed in a loop or captured by a deferred block in the initial structured-flow profile."},
    {"E0910", "optional has no valid context or operation", "null needs an expected optional type. Wrap explicitly with some(value); there is no implicit payload-to-optional conversion, truthiness or arbitrary sum equality."},
    {"E0911", "inspection profile has no tagged-value records", "The requested published inspection profile cannot describe sums, propagation or exhaustive switches. Use an explicitly sum-capable profile when available; no partial successful inventory or cached query is produced."},
    {"E0950", "read-only element contains a writable descriptor", "Read-only views may contain scalar values, inline aggregates, strings and read-only descriptors, but cannot expose an embedded writable view. Borrow individual fields or use read-only descriptors in the element."},
    {"E0951", "read-only view target is not qualified", "Read-only views are supported by the evaluator and C output. Device output profiles refuse them until their access, layout and resource semantics are qualified."},
    {"E0960", "invalid nominal ID declaration", "Use struct @id with one concrete integer field named value and no initializer. Phantom generic parameters preserve distinct domains. @id(ordered) explicitly opts into ordering. Construction and extraction remain explicit."},
    {"E0961", "invalid nominal ID operation", "Equality requires the same ID declaration on both sides. Ordering additionally requires @id(ordered). Arithmetic, bit operations, shifts and implicit conversions are unavailable: explicitly extract .value for integer arithmetic or codec conversion."},
    {"E0962", "nominal IDs in device output", "The initial nominal integer ID profile supports the evaluator and C output, including C-to-Wasm. Extract an integer in a host-side adapter before using a device profile."},
    {"E0940", "invalid effect declaration", "Use one well-formed #pure or #effects declaration with unique known capability names."},
    {"E0941", "declared effect bound is exceeded", "A concrete procedure's direct or transitive capability exceeds its declared upper bound. Inspect the original site and call path; include the capability or remove its effect."},
    {"E0942", "required effect declaration is missing", "Exported procedures, authority procedures and foreign declarations require #effects. Unannotated foreign behavior remains unknown and cannot satisfy a restricted caller."},
    {"E0943", "effect graph or workspace limit", "Effect admission could not validate its complete bounded graph and scratch storage. No partial successful contract is published."},
    {"E0970", "persisted field does not own its value", "Saved and wire records may contain scalar values, IDs, fixed arrays and owned optional or sum payloads. Views, strings and process-local handles need an owned encoding or a stable ID."},
    {"E0971", "transaction output profile is not qualified", "Undo scopes are supported by the evaluator and C output, including C-to-Wasm. Device output refuses them until its undo storage and control-flow rules are qualified."},
    {"E0974", "transaction error would publish a rolled-back reference", "A failing transaction cannot return a typed ID, handle or borrowed view in its Result error. Return an enum or owned ordinary error data, and return references only on success. Optional none carries no payload."},
    {"E0980", "realm-script directive is malformed or misplaced", "`#script \"name\";` marks its unit a realm script and `#script_api;` marks a unit that declares the script API's imports. A unit has one directive, at file scope. A script's name is 1 to 48 lowercase letters, digits and hyphens and starts with a letter. A script is the entry file of its own compilation: no other unit loads it. See coax/script-profile.md."},
    {"E0981", "realm script reaches an effect outside its profile", "Every procedure a realm script declares may only read, write and trap, directly and through what it calls: no printing, clock, native call or unchecked storage. The message names the effect and the call path that reaches it. Time and random draws come from the engine as arguments and script API calls."},
    {"E0982", "native call outside the script API", "A realm script reaches native code only through `#foreign` procedures declared in a `#script_api;` unit, each with `#effects` within foreign, reads, writes and traps. A `#foreign` in the script or in a library it loads is refused."},
    {"E0983", "realm script reaches recursion", "A realm script's stack has a known bound, so no procedure it declares may call itself, directly or through others. Use a loop over a fixed array."},
    {"E0984", "realm script exports are its handlers", "A realm script exports only handlers from its own unit: on_tick, on_enter, on_leave, on_notice, on_hit, on_use and on_timer, at least one, each taking `(me: u64, mind: [] State, ...)` with the handler's own parameters and returning nothing. It has no `main`."},
    {"E0985", "realm script state is one fixed-size record", "Every handler takes its state as a writable view of the same `#persisted` struct declared in the script: scalars, enums, IDs and fixed arrays. The engine owns the memory and keeps it across a code swap field by field."},
};

const int32_t ERROR_COUNT = (int32_t)(sizeof ERRORS / sizeof ERRORS[0]);

// One row per line: coax/web/vocab.coax reads these rows as data, so a
// spelling here is also what `coax vocab SPELLING` explains.
const Idiom IDIOMS[] = {
    {"let", "declare a local with ':=', or with its type", "x := 1;  or  x: int = 1;"},
    {"var", "declare a local with ':=', or with its type", "x := 1;  or  x: int = 1;"},
    {"const", "declare a local with ':='; a constant is 'N :: value;' at the top level", "x := 1;  or, at the top level,  N :: 1;"},
    {"auto", "declare a local with ':=', which infers its type", "x := 1;"},
    {"mut", "every local can be assigned; declare it with ':='", "x := 1;"},
    {"val", "declare a local with ':=', or with its type", "x := 1;  or  x: int = 1;"},
    {"TYPE NAME", "the type follows the name", "x: int = 1;"},
    {"elif", "write 'else if'", "if a { } else if b { }"},
    {"elsif", "write 'else if'", "if a { } else if b { }"},
    {"elseif", "write 'else if'", "if a { } else if b { }"},
    {"match", "select with switch, one case per alternative", "switch k { case Kind.a { } case Kind.b { } }"},
    {"loop", "write 'while true'", "while true { ... }"},
    {"do", "there is no do-while loop", "while true { ...; if !c { break; } }"},
    {"and", "write '&&'", "a && b"},
    {"or", "write '||'", "a || b"},
    {"not", "write '!'", "!b"},
    {"as", "convert with cast(T) x", "cast(int) x  (cast,trunc(u8) x keeps the low bits)"},
    {"(T) x", "convert with cast(T) x", "cast(int) x  (cast,trunc(u8) x keeps the low bits)"},
    {"?:", "the conditional expression is ifx", "ifx c then a else b"},
    {"->", "there are no pointers, so a field is always 'x.f'", "x.f"},
    {"(T) -> R", "a procedure is not a value, so there is no procedure type; pass an enum that names the choice and branch on it", "Op :: enum u8 { add; mul; }  apply :: (op: Op, a: int, b: int) -> int { if op == Op.add { return a + b; } return a * b; }"},
    {"++", "there is no '++'", "x += 1;"},
    {"--", "there is no '--'", "x -= 1;"},
    {"&&=", "write the assignment out", "b = b && c;"},
    {"||=", "write the assignment out", "b = b || c;"},
    {"===", "compare with '=='; values of different types never compare", "a == b"},
    {"!==", "compare with '!='; values of different types never compare", "a != b"},
    {"&x", "there are no pointers or address-of", "pass an array as a view, name[..]; to write a struct through a call, keep it in a one-element array x: [1] T and pass x[..]"},
    {"*p", "there are no pointers or dereference", "pass an array as a view, name[..], and index it"},
    {"~", "there is no '~'; flip bits with '^' and an all-ones mask of the width", "x ^ 0xFFFFFFFF  (a u32);  x & ~3 is  x & 0xFFFFFFFC"},
    {"for (;;)", "a loop runs over a half-open range or a collection", "for i: 0..<n { }  or  for x, i: items { }  or  while c { }"},
    {"for in", "a loop names its variable, then ':'", "for i: 0..<n { }  or  for x, i: items { }"},
    {"for 0..n", "a loop names its counter, and ranges are half-open", "for i: 0..<n { }"},
    {"it", "a loop names its element and index", "for x, i: items { }"},
    {"it_index", "a loop names its element and index", "for x, i: items { }"},
    {"xx", "convert with cast(T) x", "cast(int) x"},
    {"fn", "a procedure is 'name :: (parameters) -> result { }'", "add :: (a: int, b: int) -> int { return a + b; }"},
    {"func", "a procedure is 'name :: (parameters) -> result { }'", "add :: (a: int, b: int) -> int { return a + b; }"},
    {"function", "a procedure is 'name :: (parameters) -> result { }'", "add :: (a: int, b: int) -> int { return a + b; }"},
    {"def", "a procedure is 'name :: (parameters) -> result { }'", "add :: (a: int, b: int) -> int { return a + b; }"},
    {"fun", "a procedure is 'name :: (parameters) -> result { }'", "add :: (a: int, b: int) -> int { return a + b; }"},
    {"TYPE NAME(", "a procedure is 'name :: (parameters) -> result { }'", "add :: (a: int, b: int) -> int { return a + b; }"},
    {"struct", "a type is 'Name :: struct { }'", "Point :: struct { x: int; y: int; }"},
    {"class", "a type is 'Name :: struct { }'; procedures take the struct as a parameter", "Point :: struct { x: int; y: int; }"},
    {"type", "a type is 'Name :: struct { }'", "Point :: struct { x: int; y: int; }"},
    {"interface", "there are no interfaces; a generic procedure takes $T", "largest :: (xs: [] $T) -> $T { }"},
    {"enum", "an enum is 'Name :: enum WIDTH { }'", "Color :: enum u8 { red; green; }"},
    {"static", "a constant is 'N :: value;'; there are no global variables", "N :: 3;"},
    {"#define", "a constant is 'N :: value;'", "N :: 3;"},
    {"import", "load code with #import or #load", "#import \"Basic\";  or  #load \"FILE.coax\" as name;"},
    {"use", "load code with #import or #load", "#import \"Basic\";  or  #load \"FILE.coax\" as name;"},
    {"package", "a file needs no package line; load code with #import or #load", "#import \"Basic\";  or  #load \"FILE.coax\" as name;"},
    {"from", "load code with #import or #load", "#import \"Basic\";  or  #load \"FILE.coax\" as name;"},
    {"#include", "load code with #import or #load", "#import \"Basic\";  or  #load \"FILE.coax\" as name;"},
    {"pub", "every top-level name is visible to the files that load it; '#export NAME;' limits a module, '#export_c' exports to C", "add :: (a: int) -> int #export_c { return a; }"},
    {"export", "every top-level name is visible to the files that load it; '#export NAME;' limits a module, '#export_c' exports to C", "add :: (a: int) -> int #export_c { return a; }"},
    {"assert", "an assertion is the #assert statement", "#assert x == 1;"},
    {"printf", "print formats each % with the next value, from #import \"Basic\";", "print(\"x = %\\n\", x);"},
    {"println", "print formats each % with the next value, from #import \"Basic\";", "print(\"x = %\\n\", x);"},
    {"puts", "print formats each % with the next value, from #import \"Basic\";", "print(\"x = %\\n\", x);"},
    {"console", "print formats each % with the next value, from #import \"Basic\";", "print(\"x = %\\n\", x);"},
    {"strlen", "a string's length is its .count", "s.count"},
    {".len()", "there are no methods; a length is .count", "s.count"},
    {".length()", "there are no methods; a length is .count", "s.count"},
    {".size()", "there are no methods; a length is .count", "s.count"},
    {".push()", "fixed arrays do not grow; coax/lib/bounded.coax keeps a count in caller-owned storage", "bounded_push(b[..], v)"},
    {".append()", "fixed arrays do not grow; coax/lib/bounded.coax keeps a count in caller-owned storage", "bounded_push(b[..], v)"},
    {"malloc", "there is no heap; storage is a fixed array the caller owns", "buf: [4096] u8;  and pass buf[..]"},
    {"free", "there is no heap; storage is a fixed array the caller owns", "buf: [4096] u8;  and pass buf[..]"},
    {"new", "there is no heap; storage is a fixed array the caller owns", "buf: [4096] u8;  and pass buf[..]"},
    {"True", "booleans are lower case", "true"},
    {"False", "booleans are lower case", "false"},
    {"nil", "an empty optional is null, for a T? value", "x: int? = null;"},
    {"None", "an empty optional is null, for a T? value", "x: int? = null;"},
    {"NULL", "an empty optional is null, for a T? value", "x: int? = null;"},
    {"nullptr", "an empty optional is null, for a T? value", "x: int? = null;"},
    {"undefined", "an empty optional is null, for a T? value; an uninitialized local is '= ---'", "x: int? = null;  or  buf: [64] u8 = ---;"},
    {"self", "there are no methods; pass the struct as a parameter", "area :: (r: Rect) -> int { return r.w * r.h; }"},
    {"this", "there are no methods; pass the struct as a parameter", "area :: (r: Rect) -> int { return r.w * r.h; }"},
    {"i8", "signed integers are s8, s16, s32 and s64; int is s64", "s8"},
    {"i16", "signed integers are s8, s16, s32 and s64; int is s64", "s16"},
    {"i32", "signed integers are s8, s16, s32 and s64; int is s64", "s32"},
    {"i64", "signed integers are s8, s16, s32 and s64; int is s64", "s64"},
    {"usize", "counts and indices are int (s64); uint is u64", "int"},
    {"isize", "counts and indices are int (s64)", "int"},
    {"size_t", "counts and indices are int (s64); uint is u64", "int"},
    {"long", "int is s64", "int"},
    {"uint8_t", "unsigned integers are u8, u16, u32 and u64", "u8"},
    {"uint32_t", "unsigned integers are u8, u16, u32 and u64", "u32"},
    {"int32_t", "signed integers are s8, s16, s32 and s64", "s32"},
    {"float", "floats are f32 and f64", "f32"},
    {"double", "floats are f32 and f64", "f64"},
    {"boolean", "the boolean type is bool", "bool"},
    {"String", "the string type is string, an immutable view of bytes", "string"},
    {"str", "the string type is string, an immutable view of bytes", "string"},
    {"char", "a character literal 'a' is a u8", "u8"},
    {"void", "a procedure without a result has no '->'", "reset :: (x: [] int) { }"},
};

const int32_t IDIOM_COUNT = (int32_t)(sizeof IDIOMS / sizeof IDIOMS[0]);

namespace {
bool idiom_word_char(char c) { return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_'; }
bool idiom_space(char c) { return c == ' ' || c == '\t' || c == '\n' || c == '\r'; }
// The identifier that ends at or before `at`, skipping spaces back; its start in *start.
std::string idiom_word_before(const std::string &t, size_t at, size_t *start) {
  size_t e = at < t.size() ? at : t.size();
  while (e > 0 && idiom_space(t[e - 1])) e--;
  size_t b = e;
  while (b > 0 && idiom_word_char(t[b - 1])) b--;
  if (start) *start = b;
  return t.substr(b, e - b);
}
// The identifier that starts at or after `at`, skipping spaces; its end in *end.
std::string idiom_word_after(const std::string &t, size_t at, size_t *end) {
  size_t b = at;
  while (b < t.size() && idiom_space(t[b])) b++;
  size_t e = b;
  while (e < t.size() && idiom_word_char(t[e])) e++;
  if (end) *end = e;
  return t.substr(b, e - b);
}
char idiom_char_after(const std::string &t, size_t at) {
  while (at < t.size() && idiom_space(t[at])) at++;
  return at < t.size() ? t[at] : 0;
}
// The last non-space byte before `at`, or 0; its offset in *where.
char idiom_char_before(const std::string &t, size_t at, size_t *where) {
  size_t e = at < t.size() ? at : t.size();
  while (e > 0 && idiom_space(t[e - 1])) e--;
  if (where) *where = e ? e - 1 : 0;
  return e ? t[e - 1] : 0;
}
// Whether the word starting at `b` begins a statement: only spaces before it
// on its line, or a ; { } before it.
bool idiom_statement_start(const std::string &t, size_t b) {
  size_t i = b;
  while (i > 0 && (t[i - 1] == ' ' || t[i - 1] == '\t')) i--;
  if (i == 0 || t[i - 1] == '\n') return true;
  char c = idiom_char_before(t, b, nullptr);
  return c == ';' || c == '{' || c == '}';
}
bool idiom_in(const std::string &w, std::initializer_list<const char *> words) {
  for (const char *x : words) if (w == x) return true;
  return false;
}
bool idiom_type_word(const std::string &w) {
  return idiom_in(w, {"int", "uint", "float", "double", "char", "long", "short", "unsigned", "signed", "void", "bool", "boolean",
                      "string", "String", "s8", "s16", "s32", "s64", "u8", "u16", "u32", "u64", "f32", "f64", "i8", "i16",
                      "i32", "i64", "usize", "isize", "size_t", "uint8_t", "uint32_t", "int32_t"});
}
// The first 'quoted' name in a message.
std::string idiom_quoted(const std::string &m) {
  size_t a = m.find('\'');
  if (a == std::string::npos) return "";
  size_t b = m.find('\'', a + 1);
  return b == std::string::npos ? "" : m.substr(a + 1, b - a - 1);
}
const Idiom *idiom_row(const std::string &spelling) {
  for (int32_t i = 0; i < IDIOM_COUNT; i++) if (spelling == IDIOMS[i].spelling) return &IDIOMS[i];
  return nullptr;
}
bool idiom_use(const std::string &spelling, const std::string &shown, IdiomRepair *out) {
  const Idiom *row = idiom_row(spelling);
  if (!row) return false;
  out->recode = true;
  out->message = "'" + shown + "' is not Coax: " + row->note;
  out->fix = row->fix;
  return true;
}
}  // namespace

bool idiom_repair(const std::string &t, size_t o, const char *code_c, const std::string &message, IdiomRepair *out) {
  const std::string code = code_c;
  if (o > t.size()) return false;
  const char at = o < t.size() ? t[o] : 0;
  size_t wb = 0, we = 0;
  const std::string before = idiom_word_before(t, o, &wb);
  const std::string after = idiom_word_after(t, o, &we);
  const std::string name = idiom_quoted(message);
  if (code == "E0201") {
    if (message.find("loop variable") != std::string::npos) {
      if (after == "in") return idiom_use("for in", "for " + before + " in", out);
      if (idiom_char_after(t, o) == ':' && o + 1 < t.size() && idiom_char_after(t, o + 1) == '=') return idiom_use("for (;;)", "for " + before + " := ...;", out);
      return false;
    }
    const bool stmt = !before.empty() && idiom_statement_start(t, wb);
    if (stmt && !after.empty() && idiom_in(before, {"let", "var", "const", "auto", "mut", "val"})) return idiom_use(before, before, out);
    if (stmt && !after.empty() && idiom_type_word(before) && idiom_char_after(t, we) != '(') return idiom_use("TYPE NAME", before + " " + after, out);
    if (stmt && idiom_in(before, {"elif", "elsif", "elseif", "match"}) && !after.empty()) return idiom_use(before, before, out);
    if (stmt && idiom_in(before, {"loop", "do"}) && idiom_char_after(t, o) == '{') return idiom_use(before, before, out);
    if (before == "not" && !after.empty()) return idiom_use("not", "not", out);
    if (idiom_in(after, {"and", "or"})) return idiom_use(after, after, out);
    if (after == "as") return idiom_use("as", "as", out);
    char next = idiom_char_after(t, o);
    if (next == '?') return idiom_use("?:", "c ? a : b", out);
    size_t n = o;
    while (n < t.size() && idiom_space(t[n])) n++;
    // (int) -> int in a type's place: a procedure type (#13125). p->f is the pointer spelling.
    if (n + 1 < t.size() && t[n] == '-' && t[n + 1] == '>') return idiom_char_before(t, n, nullptr) == ')' ? idiom_use("(T) -> R", "(...) -> R", out) : idiom_use("->", "->", out);
    if (message.find("parenthesized type") != std::string::npos) return idiom_use("(T) -> R", "(name: T) -> R", out);
    // (T) x: a parenthesized single name, then an operand.
    size_t close = 0;
    if (idiom_char_before(t, o, &close) == ')' && (idiom_word_char(next) || next == '(')) {
      size_t ws = 0;
      const std::string inner = idiom_word_before(t, close, &ws);
      size_t open = 0;
      if (!inner.empty() && idiom_char_before(t, ws, &open) == '(') {
        size_t lead = 0;
        char c = idiom_char_before(t, open, &lead);
        if (c == '=' || c == '(' || c == ',' || idiom_word_before(t, open, nullptr) == "return")
          return idiom_use("(T) x", "(" + inner + ") " + after, out);
      }
    }
    // if c x(); from C: a block always has braces.
    size_t ls = o < t.size() ? o : t.size();
    while (ls > 0 && t[ls - 1] != '\n') ls--;
    const std::string head = t.substr(ls, o - ls);
    if (message.find("to start a block, found '") != std::string::npos && !after.empty() &&
        (head.find("if ") != std::string::npos || head.find("while ") != std::string::npos)) {
      out->fix = "a block always has braces: if c { ... }";
      return true;
    }
    return false;
  }
  if (code == "E0205") {
    if (at == '+' && ((o > 0 && t[o - 1] == '+') || (o + 1 < t.size() && t[o + 1] == '+'))) return idiom_use("++", "++", out);
    size_t p = 0;
    if (idiom_char_before(t, o, &p) == '-' && p > 0 && t[p - 1] == '-') return idiom_use("--", "--", out);
    if (at == '=' && o >= 2 && (t[o - 1] == '&' || t[o - 1] == '|') && t[o - 2] == t[o - 1]) return idiom_use(t[o - 1] == '&' ? "&&=" : "||=", t[o - 1] == '&' ? "&&=" : "||=", out);
    if (at == '=' && o >= 2 && t[o - 1] == '=' && (t[o - 2] == '=' || t[o - 2] == '!')) return idiom_use(t[o - 2] == '=' ? "===" : "!==", t[o - 2] == '=' ? "===" : "!==", out);
    if ((at == '&' || at == '*') && o + 1 < t.size() && idiom_word_char(t[o + 1]) && !(o > 0 && t[o - 1] == at)) return idiom_use(at == '&' ? "&x" : "*p", std::string(1, at) + idiom_word_after(t, o + 1, nullptr), out);
    return false;
  }
  if (code == "E0108") return at == '~' && idiom_use("~", "~", out);
  if (code == "E0208" && before == "for") {
    if (at == '(') return idiom_use("for (;;)", "for (", out);
    if (at >= '0' && at <= '9') return idiom_use("for 0..n", "for 0..n", out);
    return false;
  }
  if (code == "E0213") {
    if (idiom_in(after, {"fn", "func", "function", "def", "fun", "struct", "class", "type", "interface", "enum", "import", "use",
                         "package", "from", "pub", "export", "static"}))
      return idiom_use(after, after, out);
    if (idiom_in(after, {"const", "let", "var", "val"})) return idiom_use("static", after, out);
    if (idiom_type_word(after)) {
      size_t e2 = 0;
      const std::string second = idiom_word_after(t, we, &e2);
      if (!second.empty() && idiom_char_after(t, e2) == '(') return idiom_use("TYPE NAME(", after + " " + second + "(", out);
    }
    return false;
  }
  if (code == "E0210") {
    const std::string directive = at == '#' ? idiom_word_after(t, o + 1, nullptr) : "";
    if (directive == "include" || directive == "define") return idiom_use("#" + directive, "#" + directive, out);
    return false;
  }
  if (code == "E0204") {
    if (name == "for") return idiom_use("for 0..n", "for 0..n", out);
    if (idiom_in(name, {"it", "it_index", "xx"})) return idiom_use(name, name, out);
    return false;
  }
  if (code == "E0315") {
    if (idiom_in(name, {"assert", "printf", "println", "puts", "strlen", "malloc", "free", "new"})) return idiom_use(name, name, out);
    out->fix = "find where it is declared: coax vocab " + name;
    return !name.empty();
  }
  if (code == "E0302") {
    if (idiom_row(name) && (idiom_type_word(name) || name == "str")) return idiom_use(name, name, out);
    out->fix = "find where it is declared: coax vocab " + name;
    return !name.empty();
  }
  if (code == "E0202" && at == ')' && idiom_char_before(t, o, nullptr) == '(') return idiom_use("(T) -> R", "() -> R", out);
  if (code == "E0306") {
    if (name == "_") { out->fix = "'_' only drops a value: _ = e; or _ := e;. It is not a variable, so it can't be read or updated"; return true; }
    if (idiom_in(name, {"True", "False", "nil", "None", "NULL", "nullptr", "undefined", "self", "this"})) return idiom_use(name, name, out);
    // A name the compiler keeps for an intrinsic can't name a local, so its
    // declaration failed (E0342) and every use lands here.
    if (idiom_in(name, {"len", "log", "floor", "sin", "cos", "tan", "exp", "sqrt", "abs", "pow", "byte_at", "size_of", "type_name"}) &&
        idiom_char_after(t, o + name.size()) != '(') {
      out->fix = "'" + name + "' is the intrinsic " + name + "(...), so it can't name a local: rename the local";
      return true;
    }
    return false;
  }
  if (code == "E0384") {
    if (name == "console") return idiom_use("console", "console.log", out);
    for (const char *m : {".len(", ".length(", ".size(", ".push(", ".append("}) {
      const std::string s = m;
      if (t.compare(o + name.size(), s.size(), s) == 0) return idiom_use(s + ")", name + s + ")", out);
    }
    return false;
  }
  return false;
}

}  // namespace coax
