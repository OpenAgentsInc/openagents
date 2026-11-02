# 16. Speak the protocols

> **Status: partly implemented.** An MCP 2025-06-18 subset (the
> handshake, `tools/list` and `tools/call`) and Open Responses function
> calls run in both directions over byte strings, with typed import,
> chapter4's admission, a remote policy and a hash-chained message log.
> Coder also serves its verbs as MCP tools over the stdio transport
> (`coder mcp`, chapter 23), which Claude Code drives live. The harness's
> own client over live transports (stdio and HTTP), MCP resources, ACP for
> editors and System One as a tool remain; see [What remains](#what-remains).

Other programs already speak protocols for tools. A Model Context Protocol
(MCP) server publishes tools as JSON-RPC 2.0 messages, and the Open
Responses API from [Psionic chapter 79](../../psionic/book/ch079-serve-responses.md) names a tool call
as a `function_call` item. This chapter connects both to the harness so an
external tool is exactly as safe as a built-in one: declared with types,
admitted against a grant, and checked after it runs.

The adapter has five parts:
- **an import** that turns each tool of a `tools/list` reply into one of
  [chapter4](ch004-tools-as-typed-capabilities.md)'s `HtTool`
  declarations, and refuses a tool whose arguments or result have no typed
  counterpart instead of guessing;
- **a client** that writes a harness call, `name{arguments}`, as a
  `tools/call` request byte for byte, and reads the reply through an after
  stage that only accepts a value of the declared type;
- **a server** that answers the same requests from a registry, so the other
  direction is the same code read backwards;
- **a remote policy** in the shape of
  [chapter5](ch005-a-sandbox-and-approvals.md)'s: an effect ceiling, an
  approval for anything stronger than what runs freely, and limits on
  remote calls and their bytes;
- **a message log** that every message enters, sent or received, before
  anything acts on it, chained by SHA-256 digests.

```shell
coax/bin/coax run psionic/examples/protocols.coax
coax/bin/coax test psionic/lib/harness/protocols.coax
coax/bin/coax test psionic/tests/protocols-checks.coax
```

## The profile

`cpu-mcp-2025-06-18-subset-v1`:
- MCP protocol version `2025-06-18` over JSON-RPC 2.0, with integer request
  ids from 0 to 999999;
- the methods `initialize`, `notifications/initialized`, `tools/list` (one
  page, no `nextCursor`) and `tools/call`;
- tool schemas limited to chapter4's types: an integer from 0 to
  10^d − 1 for d from 1 to 6 (`"minimum":0`, `"maximum"` all nines), a
  boolean, or a string `enum` of 1 to 8 words; every property required; at
  most 8 tools of at most 4 fields; a result declared as the one property
  `result` of the output schema;
- one message is at most 2048 bytes;
- Responses function tools and `function_call` items in the shape chapter
  79 accepts;
- no transport: messages are byte strings passed between client and server
  in one process.

Chapter4's own profile still holds for every call: canonical JSON with
no whitespace and fields in declared order, at most 256 bytes.

## Why types, not strings

The specification's own example of a tool list declares `get_weather`
with one argument:

```text
"location":{"type":"string","description":"City name or zip code"}
```

Any string is a valid location. Chapter4's grammar can't mask a free
string down to the values a tool accepts, and its after stage can't check
one either, so this adapter refuses the tool: `untyped`, tool 0, field 0.
Refusing is the point. A tool that comes in with a type gets every
guarantee a built-in tool has; a tool that can't be typed doesn't come in.
The refusals have three kinds:
- **untyped:** the value has no typed counterpart: a free string, a
  number, an array, an object, or a tool without an output schema;
- **feature:** a schema keyword or protocol feature outside the subset,
  such as `pattern`, `default`, `$schema`, `nextCursor` or
  `additionalProperties: true`;
- **malformed:** the message isn't the shape the subset expects.

## A hand trace

Take chapter4's `get`. Its declaration is one field, `room`, a choice
of `hall`, `den` or `study`; a boolean result; effect read. Written as an
MCP tool it becomes, field by field:

| Part of `HtTool` | MCP JSON |
| --- | --- |
| name `get` | `"name":"get"` |
| field `room`, choice of 3 words | `"room":{"type":"string","enum":["hall","den","study"]}` |
| every field required | `"required":["room"]` |
| no other fields | `"additionalProperties":false` |
| result boolean | `"outputSchema":{…"properties":{"result":{"type":"boolean"}}…}` |
| effect read | `"annotations":{"readOnlyHint":true}` |

MCP has no word for a tool with no effect, so chapter4's `add` and
`div`, whose effect is none, cross as `readOnlyHint: true` and import as
read. The effect can only get stronger in a round trip, never weaker.

Import reads the same bytes back. The properties come in the order they're
written: JSON objects are unordered, but the subset fixes the order the
bytes give, because chapter4's grammar wants fields in declared order.
`required` must name each property exactly once; `["room","room"]` is a
`feature` refusal at field 0.

Now the call `get{"room":"den"}`, the next one in a session whose request id
is 3. The arguments object is the call's own bytes after the name, so the
request is three fixed pieces around two copies:

```text
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":
"get"
,"arguments":
{"room":"den"}
}}
```

That's 97 bytes. The server reads `name` and the raw `arguments` back,
joins them into `get{"room":"den"}`, and runs chapter4's `ht_call`.
`den` is room 1, and the world starts with every room off, so the value is
0. The server writes it as the tool's result object, `{"result":false}`,
twice: once as the one text content (escaped as a JSON string) and once as
`structuredContent`. The client reads `structuredContent.result` as the
declared type (`false` is 0) and requires the text content to be the same
bytes, so the model-visible text and the typed value can't disagree.

The message log starts from 32 zero bytes. Each digest is SHA-256 over the
tag `psionic.harness.protocols.message.v1` (length first, as a
little-endian u64), the previous digest, one byte for the direction (1
sent, 0 received), and the message (length first). The checks pin the
first three digests of the handshake against Python's `hashlib`.

## Write the declarations

Every writer appends to a caller's buffer and returns the new length, or
−1 when the buffer is too small; −1 stays −1 down a chain of calls, the
convention of `coax/lib/nostr/json.coax`'s writer that these reuse.

<!-- source: psionic/lib/harness/protocols.coax write -->
```jai
// The writers append to out at at and return the new length, or -1 when
// out is too small; -1 stays -1 down a chain.
pr_type_put :: (out:[]u8,at:int,t:HtType)->int {
    if t.kind==HtKind.integer {return json_raw(out,json_write_int(out,json_raw(out,at,"{\"type\":\"integer\",\"minimum\":0,\"maximum\":"),ht_power(t.digits)-1),"}");}
    if t.kind==HtKind.boolean {return json_raw(out,at,"{\"type\":\"boolean\"}");}
    n:=json_raw(out,at,"{\"type\":\"string\",\"enum\":[");
    for w,i:t.choices {if i>0 {n=json_raw(out,n,",");}n=json_write_string(out,n,w);}
    return json_raw(out,n,"]}");
}
// An object schema: every field a property, in order, and every one required.
pr_object_put :: (out:[]u8,at:int,fields:[]HtField)->int {
    n:=json_raw(out,at,"{\"type\":\"object\",\"properties\":{");
    for f,i:fields {if i>0 {n=json_raw(out,n,",");}n=pr_type_put(out,json_raw(out,json_write_string(out,n,f.name),":"),f.type);}
    n=json_raw(out,n,"},\"required\":[");
    for f,i:fields {if i>0 {n=json_raw(out,n,",");}n=json_write_string(out,n,f.name);}
    return json_raw(out,n,"],\"additionalProperties\":false}");
}
// An MCP tool. The result is the property result of the output schema. MCP
// has no word for a tool with no effect, so none crosses as read-only.
pr_tool_put :: (out:[]u8,at:int,t:HtTool)->int {
    result:=[1]HtField.[HtField.{name="result",type=t.result}];
    n:=pr_object_put(out,json_raw(out,json_write_string(out,json_raw(out,at,"{\"name\":"),t.name),",\"inputSchema\":"),t.fields);
    n=pr_object_put(out,json_raw(out,n,",\"outputSchema\":"),result[..]);
    return json_raw(out,n,ifx t.effect==HtEffect.write then ",\"annotations\":{\"readOnlyHint\":false}}" else ",\"annotations\":{\"readOnlyHint\":true}}");
}
// A JSON-RPC message's head: version and id.
pr_head :: (out:[]u8,id:int)->int {return json_write_int(out,json_raw(out,0,"{\"jsonrpc\":\"2.0\",\"id\":"),id);}
```

## Import a tool list

`pr_type` maps one property schema onto an `HtType`; `pr_object` maps an
object schema onto fields; `pr_tool` puts a tool together; `pr_import`
reads the reply's envelope and every tool. The first refusal wins and names
the tool and the field where it fell. The declarations are views into a
caller's `PrStore`, the same way chapter4's registry lives in its
caller's storage.

<!-- source: psionic/lib/harness/protocols.coax import -->
```jai
// A property's schema as one of chapter 203's types. A JSON type with no
// typed counterpart (number, array, object, a free string) is untyped; a
// keyword outside the subset is a feature.
pr_type :: (nodes:[]RsNode,i:int,words:[]string,used:int)->PrType {
    r:=PrType.{why=PrWhy.feature,used=used};
    keys:=[5]string.["type","description","minimum","maximum","enum"];
    if !rs_kind(nodes,i,JSON_OBJECT) {r.why=PrWhy.malformed;return r;}
    if !rs_keys(nodes,i,keys[..]) {return r;}
    kind:=rs_string(nodes,rs_get(nodes,i,"type"));lo:=rs_get(nodes,i,"minimum");hi:=rs_get(nodes,i,"maximum");e:=rs_get(nodes,i,"enum");
    if kind=="integer" {
        if e!= -1 || !rs_kind(nodes,lo,JSON_NUMBER) || !rs_kind(nodes,hi,JSON_NUMBER) || nodes[lo].text!="0" {return r;}
        m:=nodes[hi].text;for c:m {if c!='9' {return r;}}
        r.type=ht_int(m.count);
    }
    else if kind=="boolean" {if lo!= -1 || hi!= -1 || e!= -1 {return r;}r.type=ht_bool();}
    else if kind=="string" && e!= -1 {
        if lo!= -1 || hi!= -1 {return r;}
        if !rs_kind(nodes,e,JSON_ARRAY) {r.why=PrWhy.malformed;return r;}
        n:=used;k:=e+1;
        while k<nodes[e].next {
            if n==words.count {r.why=PrWhy.capacity;return r;}
            if !rs_kind(nodes,k,JSON_STRING) {r.why=PrWhy.malformed;return r;}
            words[n]=nodes[k].text;n+=1;k=nodes[k].next;
        }
        r.type=HtType.{kind=HtKind.choice,choices=words[used..<n]};r.used=n;
    }
    else {r.why=PrWhy.untyped;return r;}
    if ht_type(r.type) {r.why=PrWhy.none;}
    return r;
}
// An object schema's properties as fields, in the order they are written.
// Every property must be required exactly once; additionalProperties may
// only be false.
pr_object :: (nodes:[]RsNode,i:int,fields:[]HtField,words:[]string,used:int)->PrObject {
    r:=PrObject.{why=PrWhy.feature,used=used};
    keys:=[4]string.["type","properties","required","additionalProperties"];
    if !rs_kind(nodes,i,JSON_OBJECT) {r.why=PrWhy.malformed;return r;}
    if !rs_keys(nodes,i,keys[..]) {return r;}
    if rs_string(nodes,rs_get(nodes,i,"type"))!="object" {r.why=PrWhy.untyped;return r;}
    more:=rs_get(nodes,i,"additionalProperties");if more!= -1 && !rs_kind(nodes,more,JSON_FALSE) {return r;}
    p:=rs_get(nodes,i,"properties");q:=rs_get(nodes,i,"required");
    if !rs_kind(nodes,p,JSON_OBJECT) || !rs_kind(nodes,q,JSON_ARRAY) {r.why=PrWhy.malformed;return r;}
    k:=p+1;
    while k<nodes[p].next {
        r.field=r.count;name:=nodes[k].key;
        if r.count==fields.count {r.why=PrWhy.capacity;return r;}
        if !ht_word(name) {r.why=PrWhy.name;return r;}
        t:=pr_type(nodes,k,words,r.used);if t.why!=PrWhy.none {r.why=t.why;return r;}
        fields[r.count]=HtField.{name=name,type=t.type};r.used=t.used;r.count+=1;
        listed:=0;j:=q+1;while j<nodes[q].next {if rs_string(nodes,j)==name {listed+=1;}j=nodes[j].next;}
        if listed!=1 {return r;}
        k=nodes[k].next;
    }
    r.field= -1;
    if nodes[q].next-q-1!=r.count {return r;}
    r.why=PrWhy.none;return r;
}
// One MCP tool. The output schema must declare exactly one property,
// result; readOnlyHint true imports as read, anything else as write.
pr_tool :: (nodes:[]RsNode,i:int,fields:[]HtField,words:[]string,used:int,cost:int,out:[]HtTool)->PrObject {
    r:=PrObject.{why=PrWhy.malformed,used=used};
    keys:=[6]string.["name","title","description","inputSchema","outputSchema","annotations"];
    notes:=[5]string.["title","readOnlyHint","destructiveHint","idempotentHint","openWorldHint"];
    if !rs_kind(nodes,i,JSON_OBJECT) || !rs_kind(nodes,rs_get(nodes,i,"name"),JSON_STRING) {return r;}
    r.why=PrWhy.feature;if !rs_keys(nodes,i,keys[..]) {return r;}
    name:=rs_string(nodes,rs_get(nodes,i,"name"));if !ht_word(name) {r.why=PrWhy.name;return r;}
    a:=rs_get(nodes,i,"annotations");if a!= -1 && !rs_keys(nodes,a,notes[..]) {return r;}
    o:=rs_get(nodes,i,"outputSchema");if o== -1 {r.why=PrWhy.untyped;return r;}
    x:=pr_object(nodes,rs_get(nodes,i,"inputSchema"),fields,words,used);if x.why!=PrWhy.none {return x;}
    one:[1]HtField;y:=pr_object(nodes,o,one[..],words,x.used);
    if y.why==PrWhy.capacity || (y.why==PrWhy.none && (y.count!=1 || one[0].name!="result")) {y.why=PrWhy.untyped;}
    if y.why!=PrWhy.none {y.field= -1;return y;}
    effect:=ifx rs_kind(nodes,rs_get(nodes,a,"readOnlyHint"),JSON_TRUE) then HtEffect.read else HtEffect.write;
    out[0]=HtTool.{name=name,fields=fields[0..<x.count],result=one[0].type,effect=effect,cost=cost};
    x.used=y.used;return x;
}
// The JSON-RPC envelope of a reply to request id: the result node, or why not.
pr_reply :: (nodes:[]RsNode,id:int)->PrObject {
    r:=PrObject.{why=PrWhy.malformed};keys:=[4]string.["jsonrpc","id","result","error"];
    if !rs_keys(nodes,0,keys[..]) {return r;}
    if rs_string(nodes,rs_get(nodes,0,"jsonrpc"))!="2.0" {r.why=PrWhy.version;return r;}
    b:[24]u8;want:=cast(string)b[0..<json_write_int(b[..],0,id)];k:=rs_get(nodes,0,"id");
    if !rs_kind(nodes,k,JSON_NUMBER) || nodes[k].text!=want {r.why=PrWhy.id;return r;}
    if rs_get(nodes,0,"error")!= -1 {r.why=PrWhy.error;return r;}
    r.field=rs_get(nodes,0,"result");if rs_kind(nodes,r.field,JSON_OBJECT) {r.why=PrWhy.none;}
    return r;
}
// A tools/list reply to request id, imported into s: one page (no
// nextCursor), at most 8 tools, each typed or the whole import refused.
pr_import :: (reply:string,id:int,cost:int,s:[]PrStore)->PrImport {
    r:=PrImport.{why=PrWhy.malformed};
    if reply.count>PR_MESSAGE_MAX {r.why=PrWhy.shape;return r;}
    if !rs_json(reply,s[0].nodes[..],s[0].arena[..]).ok {return r;}
    nodes:=s[0].nodes[..];e:=pr_reply(nodes,id);if e.why!=PrWhy.none {r.why=e.why;return r;}
    keys:=[1]string.["tools"];if !rs_keys(nodes,e.field,keys[..]) {r.why=PrWhy.feature;return r;}
    list:=rs_get(nodes,e.field,"tools");if !rs_kind(nodes,list,JSON_ARRAY) {return r;}
    k:=list+1;used:=0;nf:=0;
    while k<nodes[list].next {
        r.tool=r.count;
        if r.count==HT_TOOLS_MAX {r.why=PrWhy.capacity;return r;}
        x:=pr_tool(nodes,k,s[0].fields[nf..<nf+HT_FIELDS_MAX],s[0].words[..],used,cost,s[0].tools[r.count..<r.count+1]);
        if x.why!=PrWhy.none {r.why=x.why;r.field=x.field;return r;}
        used=x.used;nf+=x.count;r.count+=1;k=nodes[k].next;
    }
    r.tool= -1;
    // The tools are typed one by one already; what ht_check can still find
    // is a name used twice. (No tools at all is its shape, and is allowed.)
    c:=ht_check(s[0].tools[0..<r.count]);if c.why==HtWhy.schema {r.why=PrWhy.name;r.tool=c.tool;return r;}
    r.why=PrWhy.none;return r;
}
```

## The client side of a call

`pr_check_init` refuses a server that answers `initialize` with another
protocol version: the specification lets a server propose its own version,
and lets the client disconnect when it can't speak it. `pr_result` reads a
`tools/call` reply in a fixed order: the JSON-RPC error, the content, the
tool's own error (`isError`), then the typed value.

<!-- source: psionic/lib/harness/protocols.coax client -->
```jai
// The client's three fixed requests, and the check of the server's answer
// to initialize: the same protocol version, and a tools capability.
pr_initialize :: (out:[]u8,id:int)->int {
    return json_raw(out,pr_head(out,id),",\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"psionic\",\"version\":\"210\"}}}");
}
pr_initialized :: (out:[]u8)->int {return json_raw(out,0,"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}");}
pr_list :: (out:[]u8,id:int)->int {return json_raw(out,pr_head(out,id),",\"method\":\"tools/list\"}");}
pr_check_init :: (reply:string,id:int)->PrWhy {
    nodes:[32]RsNode;arena:[512]u8;
    if !rs_json(reply,nodes[..],arena[..]).ok {return PrWhy.malformed;}
    e:=pr_reply(nodes[..],id);if e.why!=PrWhy.none {return e.why;}
    if rs_string(nodes[..],rs_get(nodes[..],e.field,"protocolVersion"))!=PR_VERSION {return PrWhy.version;}
    c:=rs_get(nodes[..],e.field,"capabilities");
    if !rs_kind(nodes[..],rs_get(nodes[..],c,"tools"),JSON_OBJECT) {return PrWhy.feature;}
    return PrWhy.none;
}
// A harness call, name{arguments}, as a tools/call request: the arguments
// object is the call's own bytes after the name.
pr_call_put :: (out:[]u8,id:int,t:HtTool,text:string)->int {
    n:=json_write_string(out,json_raw(out,pr_head(out,id),",\"method\":\"tools/call\",\"params\":{\"name\":"),t.name);
    return json_raw(out,json_raw(out,json_raw(out,n,",\"arguments\":"),text[t.name.count..<text.count]),"}}");
}
// A result as the tool returns it: {"result":value}, its declared type.
pr_value_put :: (out:[]u8,at:int,t:HtType,v:int)->int {
    n:=json_raw(out,at,"{\"result\":");
    if t.kind==HtKind.integer {n=json_write_int(out,n,v);}
    else if t.kind==HtKind.boolean {n=json_raw(out,n,ifx v==1 then "true" else "false");}
    else {n=json_write_string(out,n,t.choices[v]);}
    return json_raw(out,n,"}");
}
// The after stage for a value read from bytes: an integer of at most the
// declared digits (JSON itself refuses a leading zero), a boolean, or one
// of the words, which is exactly what chapter 203's ht_fits allows. -1 when
// it is none of these.
pr_value :: (nodes:[]RsNode,i:int,t:HtType)->int {
    if t.kind==HtKind.integer {
        if !rs_kind(nodes,i,JSON_NUMBER) {return -1;}
        x:=nodes[i].text;v:=0;
        if x.count>t.digits {return -1;}
        // A JSON number's other bytes: a sign, a point, an exponent.
        for c:x {if c=='-' || c=='.' || c=='e' || c=='E' {return -1;}v=v*10+cast(int)(c-'0');}
        return v;
    }
    if t.kind==HtKind.boolean {return ifx rs_kind(nodes,i,JSON_TRUE) then 1 else ifx rs_kind(nodes,i,JSON_FALSE) then 0 else -1;}
    for w,k:t.choices {if rs_string(nodes,i)==w {return k;}}
    return -1;
}
// A tools/call reply for tool t: a JSON-RPC error, the tool's own error
// (isError true), or structuredContent {"result":value} whose value fits
// the declared type, with the same bytes as its one text content.
pr_result :: (reply:string,id:int,t:HtTool)->PrEvent {
    r:=PrEvent.{why=PrWhy.malformed,id=id};nodes:[48]RsNode;arena:[PR_MESSAGE_MAX]u8;
    if reply.count>PR_MESSAGE_MAX {r.why=PrWhy.shape;return r;}
    if !rs_json(reply,nodes[..],arena[..]).ok {return r;}
    e:=pr_reply(nodes[..],id);if e.why!=PrWhy.none {r.why=e.why;return r;}
    keys:=[3]string.["content","structuredContent","isError"];one:=[2]string.["type","text"];result:=[1]string.["result"];
    if !rs_keys(nodes[..],e.field,keys[..]) {r.why=PrWhy.feature;return r;}
    bad:=rs_get(nodes[..],e.field,"isError");c:=rs_get(nodes[..],e.field,"content");
    if bad!= -1 && !rs_kind(nodes[..],bad,JSON_FALSE) && !rs_kind(nodes[..],bad,JSON_TRUE) {return r;}
    if !rs_kind(nodes[..],c,JSON_ARRAY) || nodes[c].next!=c+4 || !rs_keys(nodes[..],c+1,one[..]) || rs_string(nodes[..],rs_get(nodes[..],c+1,"type"))!="text" || !rs_kind(nodes[..],c+3,JSON_STRING) {return r;}
    if rs_kind(nodes[..],bad,JSON_TRUE) {r.why=PrWhy.remote;return r;}
    s:=rs_get(nodes[..],e.field,"structuredContent");
    if !rs_keys(nodes[..],s,result[..]) || s+2!=nodes[s].next {r.why=PrWhy.untyped;return r;}
    v:=pr_value(nodes[..],s+1,t.result);if v<0 {r.why=PrWhy.result;return r;}
    b:[PR_MESSAGE_MAX]u8;n:=pr_value_put(b[..],0,t.result,v);if nodes[c+3].text!=cast(string)b[0..<n] {return r;}
    r.why=PrWhy.none;r.call.value=v;return r;
}
```

## The server side

The same subset served from a registry. A call refused before it runs
answers as a protocol error, `-32602`, the code the specification gives an
unknown tool or invalid arguments, with chapter4's rendered refusal as
its message. A call that ran and failed (a division by zero, a result that
doesn't fit) answers as the tool's own error, `isError: true`.

<!-- source: psionic/lib/harness/protocols.coax server -->
```jai
// The other direction: a registry served as an MCP server. A request that
// is not JSON, not JSON-RPC 2.0 with an integer id, or not a method of the
// subset gets JSON-RPC's own error; the initialized notification gets no
// reply (0 bytes); a call goes through chapter 203's
// ht_call. A call refused before it runs is a protocol error (-32602, the
// spec's code for an unknown tool or bad arguments) with chapter 203's
// rendered refusal as its message; a call that ran and failed is the
// tool's error, isError true, with the refusal as its text.
pr_error :: (out:[]u8,id:int,code:int,message:string)->int {
    n:=ifx id== -1 then json_raw(out,0,"{\"jsonrpc\":\"2.0\",\"id\":null") else pr_head(out,id);
    n=json_write_string(out,json_raw(out,json_write_int(out,json_raw(out,n,",\"error\":{\"code\":"),code),",\"message\":"),message);
    return json_raw(out,n,"}}");
}
pr_serve :: (env:PrEnv,u:[]HtUsage,world:[]int,request:string,out:[]u8)->int {
    nodes:[64]RsNode;arena:[PR_MESSAGE_MAX]u8;keys:=[4]string.["jsonrpc","id","method","params"];
    if request.count>PR_MESSAGE_MAX || !rs_json(request,nodes[..],arena[..]).ok {return pr_error(out,-1,-32700,"Parse error");}
    k:=rs_get(nodes[..],0,"id");m:=rs_string(nodes[..],rs_get(nodes[..],0,"method"));id:=-1;
    if k== -1 && m=="notifications/initialized" {return 0;}
    if rs_kind(nodes[..],k,JSON_NUMBER) {id=pr_value(nodes[..],k,ht_int(HT_DIGITS_MAX));}
    if !rs_keys(nodes[..],0,keys[..]) || rs_string(nodes[..],rs_get(nodes[..],0,"jsonrpc"))!="2.0" || id== -1 {return pr_error(out,-1,-32600,"Invalid Request");}
    p:=rs_get(nodes[..],0,"params");
    if m=="initialize" {return json_raw(out,pr_head(out,id),",\"result\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{\"tools\":{}},\"serverInfo\":{\"name\":\"psionic\",\"version\":\"210\"}}}");}
    if m=="tools/list" {
        n:=json_raw(out,pr_head(out,id),",\"result\":{\"tools\":[");
        for t,i:env.tools {if i>0 {n=json_raw(out,n,",");}n=pr_tool_put(out,n,t);}
        return json_raw(out,n,"]}}");
    }
    if m!="tools/call" {return pr_error(out,id,-32601,"Method not found");}
    pk:=[2]string.["name","arguments"];a:=rs_get(nodes[..],p,"arguments");
    name:=rs_string(nodes[..],rs_get(nodes[..],p,"name"));
    if !rs_keys(nodes[..],p,pk[..]) || !rs_kind(nodes[..],a,JSON_OBJECT) || name.count+nodes[a].end-nodes[a].begin>HT_TEXT_MAX {return pr_error(out,id,-32602,"Invalid params");}
    b:[HT_TEXT_MAX]u8;for c,i:name {b[i]=c;}for c,i:rs_raw(request,nodes[..],a) {b[name.count+i]=c;}
    e:=ht_call(env.tools,env.grant,u,cast(string)b[0..<name.count+nodes[a].end-nodes[a].begin],world,env.work);
    t:[128]u8;rn:=ht_render(env.tools,e,t[..]);
    if !e.ran {return pr_error(out,id,-32602,cast(string)t[0..<rn]);}
    n:=json_raw(out,pr_head(out,id),",\"result\":{\"content\":[{\"type\":\"text\",\"text\":");
    if e.why!=HtWhy.none {return json_raw(out,json_write_string(out,n,cast(string)t[0..<rn]),"}],\"isError\":true}}");}
    v:[64]u8;vn:=pr_value_put(v[..],0,env.tools[e.tool].result,e.value);
    return json_raw(out,json_raw(out,json_raw(out,json_write_string(out,n,cast(string)v[0..<vn]),"}],\"structuredContent\":"),cast(string)v[0..<vn]),",\"isError\":false}}");
}
```

## Responses function calls

A model served by Psionic chapter 79 names a tool as a `function_call` item whose
`arguments` is the JSON text as a string. Unescaped, it's the same bytes
chapter4 admits, so a function call becomes a harness call by joining
the name and the arguments.

<!-- source: psionic/lib/harness/protocols.coax responses -->
```jai
// Open Responses function tools, as chapter 79 accepts them: a tool is
// {"type":"function","name":...,"parameters":schema,"strict":true}, a call
// is a function_call item whose arguments are the JSON text as a string,
// and its answer is a function_call_output with the same call_id.
pr_function_put :: (out:[]u8,at:int,t:HtTool)->int {
    n:=json_write_string(out,json_raw(out,at,"{\"type\":\"function\",\"name\":"),t.name);
    return json_raw(out,pr_object_put(out,json_raw(out,n,",\"parameters\":"),t.fields),",\"strict\":true}");
}
pr_function_call_put :: (out:[]u8,call:string,text:string)->int {
    p:=0;while p<text.count && text[p]!='{' {p+=1;}
    n:=json_write_string(out,json_raw(out,json_write_string(out,json_raw(out,0,"{\"type\":\"function_call\",\"call_id\":"),call),",\"name\":"),text[0..<p]);
    return json_raw(out,json_write_string(out,json_raw(out,n,",\"arguments\":"),text[p..<text.count]),"}");
}
// A function_call item as a harness call, name then arguments, in out.
// -1 when the item is not a function_call of the subset.
pr_function_call :: (item:string,out:[]u8)->int {
    nodes:[8]RsNode;arena:[PR_MESSAGE_MAX]u8;keys:=[4]string.["type","call_id","name","arguments"];
    if !rs_json(item,nodes[..],arena[..]).ok || !rs_keys(nodes[..],0,keys[..]) || rs_string(nodes[..],rs_get(nodes[..],0,"type"))!="function_call" {return -1;}
    for k:keys[1..<4] {if !rs_kind(nodes[..],rs_get(nodes[..],0,k),JSON_STRING) {return -1;}}
    return json_raw(out,json_raw(out,0,rs_string(nodes[..],rs_get(nodes[..],0,"name"))),rs_string(nodes[..],rs_get(nodes[..],0,"arguments")));
}
pr_function_output_put :: (out:[]u8,call:string,text:string)->int {
    n:=json_write_string(out,json_raw(out,0,"{\"type\":\"function_call_output\",\"call_id\":"),call);
    return json_raw(out,json_write_string(out,json_raw(out,n,",\"output\":"),text),"}");
}
```

## Log every message

<!-- source: psionic/lib/harness/protocols.coax log -->
```jai
// Every message, sent or received, is appended before anything acts on it.
// Its digest covers the previous digest, the direction and the bytes.
pr_digest :: (previous:[32]u8,sent:bool,text:string)->[32]u8 {
    b:[PR_MESSAGE_MAX+96]u8;at:=su_text(b[..],0,"psionic.harness.protocols.message.v1");
    for x,i:previous {b[at+i]=x;}at=su_text(b[..],su_flag(b[..],at+32,sent),text);
    return su_hash(b[0..<at]);
}
pr_start :: (g:PrLog,i:int)->int {return ifx i==0 then 0 else g.ends[i-1];}
pr_log :: (l:[]PrLog,sent:bool,text:string)->bool {
    g:=l[0];at:=pr_start(g,g.count);
    if text.count>PR_MESSAGE_MAX || g.count==g.ends.count || g.count==g.sent.count || g.count==g.heads.count || at+text.count>g.bytes.count {return false;}
    for c,i:text {g.bytes[at+i]=c;}
    previous:[32]u8;if g.count>0 {previous=g.heads[g.count-1];}
    g.ends[g.count]=at+text.count;g.sent[g.count]=sent;g.heads[g.count]=pr_digest(previous,sent,text);l[0].count=g.count+1;
    return true;
}
pr_message :: (g:PrLog,i:int)->string {return cast(string)g.bytes[pr_start(g,i)..<g.ends[i]];}
// The first message whose digest the chain does not explain, or count.
pr_verify :: (g:PrLog)->int {
    previous:[32]u8;
    for i:0..<g.count {d:=pr_digest(previous,g.sent[i],pr_message(g,i));if !su_same(d,g.heads[i]) {return i;}previous=d;}
    return g.count;
}
```

## A remote call

`pr_send` puts a call through chapter4's admission (the grant, the call
and cost budgets, the argument grammar) against the imported registry, then
the remote policy, then the log. Only then is the call charged. A refusal at
any stage sends nothing and charges nothing. `pr_receive` logs the reply
first, then reads it.

<!-- source: psionic/lib/harness/protocols.coax call -->
```jai
// A remote call. Chapter 203 admits it against the imported registry (the
// grant, the budgets, the argument grammar); the policy decides; the
// request is logged; then the call is charged. A refusal sends and charges
// nothing.
pr_send :: (env:PrEnv,s:[]PrSession,text:string,approved:bool,out:[]u8,l:[]PrLog)->PrEvent {
    x:=s[0];r:=PrEvent.{why=PrWhy.tool};p:=env.policy;
    r.call=ht_admit(env.tools,env.grant,x.usage,text,env.work);
    if r.call.why!=HtWhy.none {return r;}
    t:=env.tools[r.call.tool];
    if cast(u8)t.effect>cast(u8)p.ceiling {r.why=PrWhy.effect;}
    else if x.calls>=p.calls {r.why=PrWhy.calls;}
    else if x.bytes+text.count>p.bytes {r.why=PrWhy.bytes;}
    else if cast(u8)t.effect>cast(u8)p.allow && !approved {r.why=PrWhy.ask;}
    else {
        n:=pr_call_put(out,x.next,t,text);
        if n== -1 {r.why=PrWhy.shape;}
        else if !pr_log(l,true,cast(string)out[0..<n]) {r.why=PrWhy.capacity;}
        else {
            j:=0;while env.grant.tools[j]!=r.call.tool {j+=1;}
            s[0].usage.calls[j]+=1;s[0].usage.cost+=t.cost;s[0].calls=x.calls+1;s[0].bytes=x.bytes+text.count;s[0].next=x.next+1;
            r.why=PrWhy.none;r.id=x.next;r.call.cost=t.cost;r.call.ran=true;
        }
    }
    return r;
}
// The reply to a sent call, logged, then read through the after stage.
pr_receive :: (env:PrEnv,sent:PrEvent,reply:string,l:[]PrLog)->PrEvent {
    r:=sent;if !pr_log(l,false,reply) {r.why=PrWhy.capacity;return r;}
    x:=pr_result(reply,sent.id,env.tools[sent.call.tool]);r.why=x.why;r.call.value=x.call.value;
    return r;
}
```

The policy here is chapter5's rules in small: a ceiling no approval
passes, an approval for any effect stronger than what runs freely, and
limits on calls and bytes. It isn't chapter5's `sb_call`, which is
bound to chapter4's four built-in bodies and their ticks; see
[What remains](#what-remains).

## Run it

The [example](../../psionic/examples/protocols.coax) runs a client and a server in one process. The server holds
chapter4's registry; the client imports it through `tools/list` and
grants `get`, `set`, `add` and `div`, with reads running freely and writes
asking first.

<!-- source: psionic/examples/protocols.coax session -->
```jai
// The server: chapter 203's registry, its own grant, usage and world.
Server :: struct {env:PrEnv;usage:[1]HtUsage;world:[8]int;}
// Send one request to the server, logging the request and its reply.
exchange :: (sv:[]Server,request:string,reply:[]u8,l:[]PrLog)->string {
    #assert pr_log(l,true,request);
    n:=pr_serve(sv[0].env,sv[0].usage[..],sv[0].world[..],request,reply);
    print("  -> %\n",request);
    if n==0 {print("  (no reply)\n");return "";}
    print("  <- %\n",cast(string)reply[0..<n]);
    #assert pr_log(l,false,cast(string)reply[0..<n]);
    return cast(string)reply[0..<n];
}
// One remote call: the client's pr_send, the server, the client's pr_receive.
remote_call :: (env:PrEnv,s:[]PrSession,sv:[]Server,text:string,approved:bool,l:[]PrLog) {
    out:[PR_MESSAGE_MAX]u8;reply:[PR_MESSAGE_MAX]u8;
    print("call %",text);if approved {print(" (approved)");}print(": ");
    x:=pr_send(env,s,text,approved,out[..],l);
    if x.why!=PrWhy.none {print("refused before sending: %",why_name(x.why));if x.why==PrWhy.tool {print(" (chapter 203: %)",HT_WHY[cast(int)x.call.why]);}print("\n");return;}
    request:=pr_message(l[0],l[0].count-1);
    n:=pr_serve(sv[0].env,sv[0].usage[..],sv[0].world[..],request,reply[..]);
    y:=pr_receive(env,x,cast(string)reply[0..<n],l);
    if y.why==PrWhy.none {r:[128]u8;k:=ht_render(env.tools,y.call,r[..]);print("%\n",cast(string)r[0..<k]);}
    else {print("%\n",why_name(y.why));}
    print("  -> %\n  <- %\n",request,cast(string)reply[0..<n]);
}
```

```shell
coax/bin/coax run psionic/examples/protocols.coax
```

The transcript, [protocols.out](../../psionic/fixtures/protocols.out), compared byte
for byte in debug and release, shortened here to the calls and the
refusals:

```text
calls: grant get, set, add, div; policy: reads run, writes ask, at most 8 calls and 200 bytes
call get{"room":"den"}: {"tool":"get","result":false}
call set{"room":"den","on":true}: refused before sending: ask
call set{"room":"den","on":true} (approved): {"tool":"set","result":true}
call get{"room":"den"}: {"tool":"get","result":true}
call add{"a":37,"b":48}: {"tool":"add","result":85}
call div{"a":7,"b":0}: remote
call add{"a":999,"b":1}: remote
call get{"room":"attic"}: refused before sending: tool (chapter 203: arguments)
call mul{"a":1,"b":2}: refused before sending: tool (chapter 203: unknown)
call get{"room":"hall"}: {"tool":"get","result":false}
  session: 7 calls, 131 bytes, cost 7
refusals
  the specification's tools/list example: feature
  the same, one page: untyped, tool 0
  with an output schema: untyped, tool 0, field 0
  a server on 2024-11-05: version
  the specification's error reply: error
  the specification's failed call: remote
  the specification's tools/call example, served: {"jsonrpc":"2.0","id":2,"error":{"code":-32602,"message":"{\"error\":\"unknown\",\"at\":0}"}}
  set under a read-only ceiling: effect
log: 21 messages, 11 sent, chain verified through 21, head ad31e7e949cc25fd
```

The first `set` stops at `ask`: its effect is write, above what runs
freely, and nothing is sent. Approved, it runs, and the next `get` sees the
den on. `div` by zero and `999 + 1` (which doesn't fit 3 digits) are
refused by the server's run and after stages, and both come back as the
tool's own error, `remote`. `attic` isn't a room and `mul` isn't a tool, so
chapter4 refuses both before the client writes a byte. The session
counts 7 calls and 131 bytes: the seven calls that were sent.

## Refusals

Each refusal in the transcript is a deliberately broken input:
- the specification's `tools/list` example has `nextCursor`, so it's a
  second page the subset doesn't follow: `feature`;
- the same list as one page: its tool has no output schema, so the result
  has no type: `untyped`, tool 0;
- with an output schema added, the free-string `location` is still
  `untyped`, at field 0;
- a server that answers `initialize` with `2024-11-05`: `version`;
- the specification's JSON-RPC error reply: `error`;
- the specification's failed call, `isError: true`: `remote`;
- the specification's `tools/call` example sent to this server: `-32602`
  with chapter4's refusal, `{"error":"unknown","at":0}`;
- `set` under a read-only ceiling, even approved: `effect`.

The checks in `psionic/lib/harness/protocols.coax` cover the rest, one
refusal at a time: every keyword outside the subset, each JSON type with no
typed counterpart, integers whose bounds aren't 0 and all nines, enums
that are empty, too long, repeated or not words, five properties where
four fit, nine tools where eight fit, and a message of 2049 bytes. A
`tools/call` reply is refused when its text content and its
`structuredContent` disagree, when content holds two items or an image,
or when the value is `1000` for a 3-digit result, `-1`, `8.5`, `"85"` or
`true`.

## Independent checks

The fixtures in `psionic/tests/harness-protocols-fixtures.coax` are written
by hand from the specification's message shapes (the lifecycle and tools
pages of 2025-06-18) for chapter4's registry, and the `PR_FX_SPEC_*`
messages are the specification's own examples. The checks require:
- every writer to reproduce its fixture byte for byte, and to return −1
  with one byte less room;
- the fixture tool list to import as chapter4's registry, field by field
  and type by type, and to export again to the same bytes;
- the server to answer each fixture request with its fixture reply;
- a `function_call` item to become the harness call and back, byte for
  byte;
- the log's first three digests to equal Python `hashlib`'s over the same
  encoding.

[protocols-checks.coax](../../psionic/tests/protocols-checks.coax) adds a `#forall`
over 40 random registries from chapter4's own enumeration oracle
(`psionic/tests/harness-tools-oracle.coax`): each registry is served,
listed, imported and exported again to the same bytes, and every call the
oracle writes goes client to server to client and must give the same
outcome as calling chapter4's `ht_call` directly.

## What remains

- **Live transports.** The harness's messages are byte strings in one
  process. Coder's `coder mcp` serves the stdio transport for real
  (newline-delimited messages; Claude Code connects to it), but the
  harness's client and server here don't use it yet, and the Streamable
  HTTP transport needs the host's socket effects.
- **The rest of MCP.** Resources, prompts, sampling, pagination with
  `nextCursor`, notifications other than `initialized`, string request ids,
  and content other than one text item.
- **Chapter5's sandbox itself.** Remote calls go through this chapter's
  policy, not `sb_call`, whose approvals, pattern grants, simulated clock
  and replay are bound to chapter4's four built-in bodies.
- **ACP for editors.** No editor session runs over the Agent Client
  Protocol yet.
- **System One as a tool.** A `/v1/systemone` decision from
  [Psionic chapter 20](../../psionic/book/ch020-serve-systemone.md) isn't yet offered as a typed tool.
- **OpenAgents wire contracts.** This chapter speaks third-party protocols
  only. A protocol OpenAgents publishes starts in `proto/` and
  `docs/protocol/` with a version.

## Sources

- [Chapter plan](PLAN.md); moved from Part XXI of the [Psionic book](../../psionic/book/README.md) on 2026-11-01.
- Model Context Protocol specification, version 2025-06-18: the lifecycle
  page (`initialize`, version negotiation, `notifications/initialized`) and
  the tools page (`tools/list`, `tools/call`, `inputSchema`, `outputSchema`,
  `structuredContent`, `isError`, annotations, and the error codes).
- JSON-RPC 2.0 specification: request, response and error objects, and the
  codes −32700, −32600, −32601 and −32602.
- [Harness notes from DeepSeek Harness](../../psionic/docs/harness.md).
