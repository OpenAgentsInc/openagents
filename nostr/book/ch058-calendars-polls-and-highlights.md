# 58. Calendars, polls and highlights

Three NIPs put structured content in tags: calendar events and the replies
to them, polls and their votes, and highlighted passages of text. This
chapter reads and writes each of them, checks dates and times, and counts a
poll's votes exactly as NIP-88 says.

Part X, Every NIP. Result: NIP-52 date (31922) and time (31923) events,
calendars (31924) and RSVPs (31925); NIP-88 polls (1068) and responses (1018);
NIP-84 highlights (9802).

## What this chapter teaches

- NIP-52: `start` and `end` as ISO dates (`YYYY-MM-DD`) on kind 31922 and as
  Unix seconds on kind 31923, with `end` exclusive and after `start`; the
  `D` day tags and `start_tzid`/`end_tzid`; calendars that list events;
  RSVPs with status `accepted`, `declined` or `tentative` and an optional
  `fb` of `free` or `busy`.
- NIP-88: `option` tags with alphanumeric ids, `polltype` `singlechoice` or
  `multiplechoice`, `endsAt`, and `response` tags.
- The NIP-88 tally: one counted response per pubkey, the latest one at or
  before `endsAt`; option ids the poll doesn't define are ignored; a
  singlechoice poll counts the first valid option.
- NIP-84: kind 9802 with the highlighted text, its source in `e`, `a`, `i`
  or `r` tags, credited authors in `p` tags, and optional `context` and
  `comment`.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip32.coax](../../coax/lib/nostr/nip32.coax) | `n32_tag(e, i)` | Reads tag `i` of any name: the name and its first two values (chapter 55) |
| [tags.coax](../../coax/lib/nostr/tags.coax) | `nostr_tag(e, i)` | Reads an `e`, `p` or `a` tag and checks its values (chapter 22) |
| [nip52.coax](../../coax/lib/nostr/nip52.coax) | `n52_date(s)` | An ISO date as a day number counted from 1970-01-01 |
| [nip52.coax](../../coax/lib/nostr/nip52.coax) | `n52_read(e)` | A date or time calendar event: title, start, end, time zones, `D` tags |
| [nip52.coax](../../coax/lib/nostr/nip52.coax) | `n52_calendar(e)` | A calendar and the events it lists |
| [nip52.coax](../../coax/lib/nostr/nip52.coax) | `n52_rsvp(e)` | An RSVP: the event it answers, the answer and `fb` |
| [nip52.coax](../../coax/lib/nostr/nip52.coax) | `n52_rsvp_tags(address, d, answer, fb, out)` | Writes an RSVP's tags |
| [nip88.coax](../../coax/lib/nostr/nip88.coax) | `n88_poll(e)` | A poll's options, type and end time |
| [nip88.coax](../../coax/lib/nostr/nip88.coax) | `n88_response(e)` | A response: the poll it names and the option ids it chooses |
| [nip88.coax](../../coax/lib/nostr/nip88.coax) | `n88_response_tags(poll, choices, out)` | Writes a response's tags |
| [nip88.coax](../../coax/lib/nostr/nip88.coax) | `n88_tally(poll, set)` | The poll's result over a set of events |
| [nip84.coax](../../coax/lib/nostr/nip84.coax) | `n84_read(e)` | A highlight's source, authors, mentions, context and comment |

All three library files are new with this chapter. `nip88.coax` is an
`#authority` file, because its tally decides which votes count. The other
two are not marked `#authority`, but they were mutation-tested the same way
and no mutants survived. None of the three fetches an event or verifies a
signature. The caller verifies the events it passes (chapter 21).

## Calendar events

A calendar event is addressable: its `d` tag names it, and a newer event
with the same kind, author and `d` replaces it (chapter 22). Both kinds
need `d`, `title` and `start`. A date event (31922) counts whole days, so
its `start` and `end` are ISO dates. A time event (31923) has `start` and
`end` in Unix seconds. In both, `end` is exclusive and must be greater than
`start`. Without `end`, a date event covers its start date, and a time event
is an instant.

The reader returns a record whose `start` and `end` are day numbers for a
date event and seconds for a time event, so one comparison checks both
kinds.

<!-- source: coax/lib/nostr/nip52.coax record -->
```jai
N52Date :: struct {ok:bool; day:int;}        // days since 1970-01-01
// A calendar event, kind 31922 or 31923. start and end are day numbers for
// 31922 and Unix seconds for 31923; end is exclusive.
N52Event :: struct {
  status:int=N52_EVENT; at:int=-1; tag_error:int; missing:int=-1;
  kind:int; d:N32Text; title:N32Text;
  start:int; end:int; has_end:bool;
  start_tzid:N32Text; end_tzid:N32Text;      // end_tzid defaults to start_tzid
  days:int;                                  // D tags (31923)
  participants:int;                          // p tags
  calendars:int;                             // a tags: requests to join a 31924
}
// An addressable event a calendar or an RSVP points at.
N52Ref :: struct {kind:int; author:[32]u8; d:N32Text;}
N52Calendar :: struct {
  status:int=N52_EVENT; at:int=-1; tag_error:int; missing:int=-1;
  d:N32Text; title:N32Text;
  events:[N52_MAX_REFS]N52Ref; n:int;        // a tags to 31922 or 31923
}
N52Rsvp :: struct {
  status:int=N52_EVENT; at:int=-1; tag_error:int; missing:int=-1;
  d:N32Text; to:N52Ref;                      // the first a tag
  has_revision:bool; revision:[32]u8;        // the e tag: one revision
  answer:int=-1;                             // accepted, declined or tentative
  fb:int=-1;                                 // free, busy or absent
  has_author:bool; author:[32]u8;            // the p tag
}
```

### Dates as day numbers

`n52_date` accepts exactly `YYYY-MM-DD`: ten bytes, digits, and two
hyphens. The month must be 1 to 12, and the day must exist in that month. A
leap year is divisible by 4, except a century not divisible by 400, so
2024-02-29 and 2000-02-29 exist and 2026-02-29 and 1900-02-29 do not.

A valid date becomes the number of days since 1970-01-01. The calculation
starts each year on 1 March, so the leap day falls at the end of the year
and the month lengths before it are fixed. Shifting the year by 400 keeps
every number positive: 400 Gregorian years are exactly 146097 days.

<!-- source: coax/lib/nostr/nip52.coax date -->
```jai
n52_digits :: (s:string,from:int,to:int)->int {
  v:=0;
  for i:from..<to {if s[i]<'0' || s[i]>'9' {return -1;}v=v*10+cast(int)(s[i]-'0');}
  return v;
}
// YYYY-MM-DD as a day number counted from 1970-01-01, or ok false. The
// month's length counts leap years: every fourth year, except centuries
// not divisible by 400.
n52_date :: (s:string)->N52Date {
  r:N52Date;
  if s.count!=10 || s[4]!='-' || s[7]!='-' {return r;}
  y:=n52_digits(s,0,4);m:=n52_digits(s,5,7);d:=n52_digits(s,8,10);
  if y<0 || m<1 || m>12 || d<1 {return r;}
  leap:=y%4==0 && (y%100!=0 || y%400==0);
  length:=ifx m==2 then (ifx leap then 29 else 28) else ifx m==4 || m==6 || m==9 || m==11 then 30 else 31;
  if d>length {return r;}
  // Count from 1 March so February, with its leap day, ends the year:
  // March is month 0 and a year runs 365 days plus its leap rule.
  yy:=ifx m<=2 then y-1+400 else y+400;   // shifted 400 years: never negative
  mm:=ifx m<=2 then m+9 else m-3;
  days_in_year:=(153*mm+2)/5+d-1;
  days:=yy*365+yy/4-yy/100+yy/400+days_in_year;
  r.ok=true;r.day=days-719468-146097;return r;
}
```

### Worked example: 2026-11-20

1. The shape matches: `2026`, `-`, `11`, `-`, `20`. November has 30 days,
   so day 20 exists.
2. November is after February, so `yy = 2026 + 400 = 2426` and
   `mm = 11 - 3 = 8`.
3. `days_in_year = (153 × 8 + 2) / 5 + 20 − 1 = 1226 / 5 + 19 = 245 + 19 = 264`,
   with integer division. 245 is the number of days from 1 March to
   1 November.
4. `days = 2426 × 365 + 2426/4 − 2426/100 + 2426/400 + 264
   = 885490 + 606 − 24 + 6 + 264 = 886342`.
5. `day = 886342 − 719468 − 146097 = 20777`. The constant 719468 is the
   day number of 1970-01-01 in this count, and 146097 removes the
   400-year shift.

So 2026-11-20 is day 20777, and 2026-11-22 is day 20779. A meetup from
2026-11-20 with `end` 2026-11-22 covers days 20777 and 20778: two days,
because `end` is exclusive. Python's `datetime.date` gives the same two
numbers.

### Reading a calendar event

`n52_read` walks the tags once. The first `d`, `title`, `start` and `end`
count. The deprecated `name` tag is used as the title only when there is no
`title`. On a time event, `start_tzid` and `end_tzid` must look like IANA
names (letters, digits, `_`, `/`, `+` and `-`), and `end_tzid` defaults to
`start_tzid`. NIP-52 requires at least one `D` tag, `floor(start / 86400)`,
on a time event. This reader requires every `D` tag to be a number and one
of them to be the start's day. A `p` tag names a participant. An `a` tag
must point at a calendar (kind 31924): that is a request to be added to
it.

<!-- source: coax/lib/nostr/nip52.coax read -->
```jai
// A calendar event of kind 31922 (dates) or 31923 (times).
n52_read :: (e:NostrEvent)->N52Event {
  r:N52Event;if e.error!=NE_OK {return r;}
  if e.kind!=N52_DATE_KIND && e.kind!=N52_TIME_KIND {return n52_refuse(r,N52_KIND,-1);}
  r.kind=e.kind;dated:=e.kind==N52_DATE_KIND;
  has_d:=false;has_title:=false;has_start:=false;end_at:int;
  name:N32Text;day_tags:[1]int;   // where the D tags start
  has_end_zone:=false;
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n52_refuse(r,N52_LIMIT,i);}
    if t.error!=NT_OK {return n52_refuse(r,N52_EVENT,i);}
    tag:=t.v[0];vt:=t.v[1];value:=cast(string)vt.b[0..<vt.n];
    if n32_is(tag,"d") && !has_d {has_d=true;r.d=t.v[1];}
    else if n32_is(tag,"title") && !has_title {has_title=true;r.title=t.v[1];}
    else if n32_is(tag,"name") && name.n==0 {name=t.v[1];}
    else if (n32_is(tag,"start") && !has_start) || (n32_is(tag,"end") && !r.has_end) {
      v:=ifx dated then n52_date(value).day else n52_seconds(value);
      bad:=ifx dated then !n52_date(value).ok else v<0;
      if bad {return n52_refuse(r,ifx dated then N52_DATE else N52_TIME,i);}
      if n32_is(tag,"start") {has_start=true;r.start=v;} else {r.has_end=true;r.end=v;end_at=i;}
    }
    else if !dated && (n32_is(tag,"start_tzid") || n32_is(tag,"end_tzid")) {
      if !n52_zone(value) {return n52_refuse(r,N52_ZONE,i);}
      if n32_is(tag,"start_tzid") {r.start_tzid=t.v[1];} else {r.end_tzid=t.v[1];has_end_zone=true;}
    }
    else if !dated && n32_is(tag,"D") {
      if n52_seconds(value)<0 {return n52_refuse(r,N52_DAY,i);}
      r.days+=1;
    }
    else if n32_is(tag,"p") || n32_is(tag,"a") {
      s:=nostr_tag(e,i);
      if s.error!=NT_OK {r.tag_error=s.error;return n52_refuse(r,N52_TAG,i);}
      if s.type==NT_A && s.address_kind!=N52_CALENDAR {return n52_refuse(r,N52_REFERENCE,i);}
      if s.type==NT_A {r.calendars+=1;} else {r.participants+=1;}
    }
    i+=1;
  }
  if !has_d {return n52_need(r,N52_NEED_D);}
  if r.title.n==0 {r.title=name;}
  if r.title.n==0 {return n52_need(r,N52_NEED_TITLE);}
  if !has_start {return n52_need(r,N52_NEED_START);}
  if r.has_end && r.end<=r.start {return n52_refuse(r,N52_ORDER,end_at);}
  if !r.has_end {r.end=ifx dated then r.start+1 else r.start;}
  if !dated {
    if r.days==0 {return n52_need(r,N52_NEED_DAY);}
    if !n52_has_day(e,r.start/N52_DAY_SECONDS) {return n52_refuse(r,N52_DAY,-1);}
    if !has_end_zone {r.end_tzid=r.start_tzid;}
  }
  r.status=N52_OK;return r;
}
// Whether one of the event's D tags is this day.
n52_has_day :: (e:NostrEvent,day:int)->bool {
  i:=0;t:=n32_tag(e,i);
  while t.error==NT_OK {
    vt:=t.v[1];
    if n32_is(t.v[0],"D") && n52_seconds(cast(string)vt.b[0..<vt.n])==day {return true;}
    i+=1;t=n32_tag(e,i);
  }
  return false;
}
```

The refusals and the tag index they report:

- `N52_ORDER`: `end` is not greater than `start`. The index is the `end`
  tag's.
- `N52_DATE` and `N52_TIME`: a `start` or `end` that is not a valid date, or
  not non-negative decimal seconds.
- `N52_ZONE`: a time zone name with other characters, or empty.
- `N52_DAY`: a `D` tag that is not a number (at that tag), or no `D` tag
  for the start's day (index −1).
- `N52_MISSING` with `missing` naming `d`, `title`, `start` or `D`.
- `N52_TAG` and `N52_REFERENCE`: a bad `p` or `a` tag, or an `a` tag to
  something other than a calendar.

## Calendars and RSVPs

A calendar (31924) has a `d`, a `title` and `a` tags that list calendar
events. Each `a` tag must point at a kind 31922 or 31923 event.

<!-- source: coax/lib/nostr/nip52.coax calendar -->
```jai
// A calendar, kind 31924: its d, its title and the events it lists.
n52_calendar :: (e:NostrEvent)->N52Calendar {
  c:N52Calendar;if e.error!=NE_OK {return c;}
  if e.kind!=N52_CALENDAR {c.status=N52_KIND;return c;}
  has_d:=false;i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error!=NT_OK {c.status=ifx t.error==NT_LIMIT then N52_LIMIT else N52_EVENT;c.at=i;return c;}
    if n32_is(t.v[0],"d") && !has_d {has_d=true;c.d=t.v[1];}
    else if n32_is(t.v[0],"title") && c.title.n==0 {c.title=t.v[1];}
    else if n32_is(t.v[0],"a") {
      x:=n52_ref(e,i);
      if x.status!=N52_OK {c.status=x.status;c.tag_error=x.tag_error;c.at=i;return c;}
      if c.n==N52_MAX_REFS {c.status=N52_LIMIT;c.at=i;return c;}
      c.events[c.n]=x.ref;c.n+=1;
    }
    i+=1;
  }
  if !has_d {c.status=N52_MISSING;c.missing=N52_NEED_D;return c;}
  if c.title.n==0 {c.status=N52_MISSING;c.missing=N52_NEED_TITLE;return c;}
  c.status=N52_OK;return c;
}
```

An RSVP (31925) answers one calendar event. It needs an `a` tag to the
event, a `d` tag and a `status`. An `e` tag may name the revision it
answers, and a `p` tag the event's author. `status` must be `accepted`,
`declined` or `tentative`. `fb` must be `free` or `busy`, and NIP-52 says
it must be omitted or ignored when the status is `declined`. This reader
ignores it then, even when its value is not `free` or `busy`.

<!-- source: coax/lib/nostr/nip52.coax rsvp -->
```jai
// An RSVP, kind 31925: the calendar event it answers, and the answer.
n52_rsvp :: (e:NostrEvent)->N52Rsvp {
  r:N52Rsvp;if e.error!=NE_OK {return r;}
  if e.kind!=N52_RSVP {r.status=N52_KIND;return r;}
  has_d:=false;has_a:=false;fb_at:= -1;fb:N32Text;i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error!=NT_OK {r.status=ifx t.error==NT_LIMIT then N52_LIMIT else N52_EVENT;r.at=i;return r;}
    tag:=t.v[0];
    if n32_is(tag,"d") && !has_d {has_d=true;r.d=t.v[1];}
    else if n32_is(tag,"a") && !has_a {
      x:=n52_ref(e,i);
      if x.status!=N52_OK {r.status=x.status;r.tag_error=x.tag_error;r.at=i;return r;}
      has_a=true;r.to=x.ref;
    }
    else if (n32_is(tag,"e") && !r.has_revision) || (n32_is(tag,"p") && !r.has_author) {
      s:=nostr_tag(e,i);
      if s.error!=NT_OK {r.status=N52_TAG;r.tag_error=s.error;r.at=i;return r;}
      if s.type==NT_E {r.has_revision=true;r.revision=s.id;} else {r.has_author=true;r.author=s.pubkey;}
    }
    else if n32_is(tag,"status") && r.answer<0 {
      vt:=t.v[1];r.answer=n52_answer(cast(string)vt.b[0..<vt.n]);
      if r.answer<0 {r.status=N52_STATUS;r.at=i;return r;}
    }
    else if n32_is(tag,"fb") && fb_at<0 {fb_at=i;fb=t.v[1];}
    i+=1;
  }
  if !has_d {r.status=N52_MISSING;r.missing=N52_NEED_D;return r;}
  if !has_a {r.status=N52_MISSING;r.missing=N52_NEED_A;return r;}
  if r.answer<0 {r.status=N52_MISSING;r.missing=N52_NEED_STATUS;return r;}
  // fb is ignored when the answer is declined.
  if fb_at>=0 && r.answer!=N52_DECLINED {
    f:=fb;r.fb=n52_fb(cast(string)f.b[0..<f.n]);
    if r.fb<0 {r.status=N52_FB;r.at=fb_at;return r;}
  }
  r.status=N52_OK;return r;
}
```

The writer produces an RSVP's tags. It refuses an address that is not a
calendar event's and an answer or `fb` out of range. It leaves `fb` out for
a declined answer, so a reader never has a value to ignore.

<!-- source: coax/lib/nostr/nip52.coax write -->
```jai
// The tags of an RSVP to the calendar event at address: its a, d, status
// and, unless declined or fb is -1, fb. The bytes written, or -1 when the
// address is not a calendar event's, an answer is out of range or out is
// too small.
n52_rsvp_tags :: (address:string,d:string,answer:int,fb:int,out:[]u8)->int {
  if answer<0 || answer>2 || fb< -1 || fb>1 {return -1;}
  probe:=nt_address(address);
  if probe.error!=NT_OK || (probe.address_kind!=N52_DATE_KIND && probe.address_kind!=N52_TIME_KIND) {return -1;}
  n:=json_raw(out,0,"[[\"a\",");n=json_write_string(out,n,address);
  n=json_raw(out,n,"],[\"d\",");n=json_write_string(out,n,d);
  n=json_raw(out,n,"],[\"status\",");n=json_write_string(out,n,N52_STATUSES[answer]);
  if fb>=0 && answer!=N52_DECLINED {n=json_raw(out,n,"],[\"fb\",");n=json_write_string(out,n,N52_FBS[fb]);}
  return json_raw(out,n,"]]");
}
```

## Polls

A poll (1068) has its question in the content and one `option` tag per
choice: an id, then a label. NIP-88 says the id is alphanumeric. This
reader requires that, and refuses an id that repeats, because a vote for it
would be ambiguous. A poll without `polltype` is singlechoice. `endsAt` is
Unix seconds. `relay` tags name where to send responses; the reader counts
them.

<!-- source: coax/lib/nostr/nip88.coax record -->
```jai
N88Option :: struct {id:N32Text; label:N32Text;}
N88Poll :: struct {
  status:int=N88_EVENT; at:int=-1;            // the refusing tag
  options:[N88_MAX_OPTIONS]N88Option; n:int;
  single:bool=true;                           // no polltype: singlechoice
  has_end:bool; ends_at:int;                  // endsAt, Unix seconds
  relays:int;                                 // relay tags
}
N88Response :: struct {
  status:int=N88_EVENT; at:int=-1;
  poll:[32]u8;                                // the first e tag
  choices:[N88_MAX_CHOICES]N32Text; n:int;    // response tags, in order
}
N88Tally :: struct {
  status:int=N88_OK;                          // the poll's, when it doesn't read
  counts:[N88_MAX_OPTIONS]int;                // votes per option, in poll order
  voters:int;                                 // pubkeys with a counted response
  blank:int;                                  // counted responses naming no option
  superseded:int;                             // older responses by the same pubkey
  late:int;                                   // responses after endsAt
  ignored:int;                                // option ids the poll doesn't define
  repeated:int;                               // a response already in the set
  refused:int;                                // kind 1018 events that don't read
}
```

<!-- source: coax/lib/nostr/nip88.coax poll -->
```jai
// A poll: its options in order, its type and its end time.
n88_poll :: (e:NostrEvent)->N88Poll {
  p:N88Poll;if e.error!=NE_OK {return p;}
  if e.kind!=N88_KIND_POLL {return n88_refuse(p,N88_KIND,-1);}
  typed:=false;i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n88_refuse(p,N88_LIMIT,i);}
    if t.error!=NT_OK {return n88_refuse(p,N88_EVENT,i);}
    tag:=t.v[0];vt:=t.v[1];value:=cast(string)vt.b[0..<vt.n];
    if n32_is(tag,"option") {
      if !n88_alnum(value) {return n88_refuse(p,N88_OPTION,i);}
      if n88_index(p,value)>=0 {return n88_refuse(p,N88_DUPLICATE,i);}
      if p.n==N88_MAX_OPTIONS {return n88_refuse(p,N88_LIMIT,i);}
      p.options[p.n].id=t.v[1];p.options[p.n].label=t.v[2];p.n+=1;
    } else if n32_is(tag,"polltype") && !typed {
      typed=true;
      if value!="singlechoice" && value!="multiplechoice" {return n88_refuse(p,N88_TYPE,i);}
      p.single=value=="singlechoice";
    } else if n32_is(tag,"endsAt") && !p.has_end {
      v:=json_int(value);
      if !v.ok || v.value<0 {return n88_refuse(p,N88_TIME,i);}
      p.has_end=true;p.ends_at=v.value;
    } else if n32_is(tag,"relay") {p.relays+=1;}
    i+=1;
  }
  if p.n==0 {return n88_refuse(p,N88_MISSING,-1);}
  p.status=N88_OK;return p;
}
// The index of the option with this id, or -1.
n88_index :: (p:N88Poll,id:string)->int {
  for k:0..<p.n {if n32_is(p.options[k].id,id) {return k;}}
  return -1;
}
```

A response (1018) names the poll in its first `e` tag and lists option ids
in `response` tags, in order. It needs both. The reader keeps every
`response` value, valid or not: only the poll knows which ids are valid.

<!-- source: coax/lib/nostr/nip88.coax response -->
```jai
// A response: the poll its first e tag names and the option ids it chooses.
n88_response :: (e:NostrEvent)->N88Response {
  r:N88Response;if e.error!=NE_OK {return r;}
  if e.kind!=N88_KIND_RESPONSE {r.status=N88_KIND;return r;}
  has_poll:=false;i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error!=NT_OK {r.status=ifx t.error==NT_LIMIT then N88_LIMIT else N88_EVENT;r.at=i;return r;}
    if n32_is(t.v[0],"e") && !has_poll {
      s:=nostr_tag(e,i);
      if s.error!=NT_OK {r.status=N88_TAG;r.at=i;return r;}
      has_poll=true;r.poll=s.id;
    } else if n32_is(t.v[0],"response") {
      if r.n==N88_MAX_CHOICES {r.status=N88_LIMIT;r.at=i;return r;}
      r.choices[r.n]=t.v[1];r.n+=1;
    }
    i+=1;
  }
  if !has_poll || r.n==0 {r.status=N88_MISSING;return r;}
  r.status=N88_OK;return r;
}
```

<!-- source: coax/lib/nostr/nip88.coax write -->
```jai
// The tags of a response to the poll with this id choosing these option
// ids, in order. The bytes written, or -1 for no choices, more than
// N88_MAX_CHOICES, or too little room.
n88_response_tags :: (poll:[32]u8,choices:[]string,out:[]u8)->int {
  if choices.count==0 || choices.count>N88_MAX_CHOICES {return -1;}
  hex:[64]u8;id:=poll;h:=hex_encode_checked(cast(string)id[..],hex[..],false);
  n:=json_raw(out,0,"[[\"e\",\"");n=json_raw(out,n,cast(string)hex[..]);n=json_raw(out,n,"\"]");
  for c:choices {n=json_raw(out,n,",[\"response\",");n=json_write_string(out,n,c);n=json_raw(out,n,"]");}
  return json_raw(out,n,"]");
}
```

## Counting votes

NIP-88 gives the counting rule in three parts:

1. Count only responses within the poll's limits: at or before `endsAt`
   when the poll has one. NIP-88's example filter uses `until: endsAt`,
   which includes `endsAt` itself.
2. Count one response per pubkey: the one with the largest `created_at`.
3. Read that response by the poll type. A singlechoice poll takes the first
   response tag. A multiplechoice poll takes the first tag for each id.

The NIP leaves three cases open, and this tally decides them:

- **Ties.** NIP-88's example code keeps whichever of two equal timestamps
  it saw first, so its result depends on arrival order. This tally breaks
  the tie with the lower id, as NIP-01 does for replaceable events.
- **Invalid ids.** An id the poll doesn't define is ignored, and a
  singlechoice poll takes the first *valid* id. A response that names no
  valid id is blank: it still replaces its author's earlier responses.
- **Late responses.** A response after `endsAt` is dropped before the
  pubkey rule runs, so a late vote never replaces an earlier one that
  counted.

The same event delivered twice counts once (`repeated`). A response that
doesn't read is counted as `refused`; a response to another poll is passed
over.

<!-- source: coax/lib/nostr/nip88.coax tally -->
```jai
// Whether a response counts toward the poll: it reads, names the poll, and
// is not after endsAt.
n88_counts :: (e:NostrEvent,poll_id:[32]u8,p:N88Poll)->bool {
  if e.kind!=N88_KIND_RESPONSE {return false;}
  r:=n88_response(e);
  return r.status==N88_OK && ns_equal32(r.poll,poll_id) && (!p.has_end || e.created_at<=p.ends_at);
}
// Whether b replaces a: later, or as late with a lower id.
n88_newer :: (b:NostrEvent,a:NostrEvent)->bool {
  if b.created_at>a.created_at {return true;}
  if b.created_at<a.created_at {return false;}
  for i:0..<32 {
    if b.id[i]<a.id[i] {return true;}
    if b.id[i]>a.id[i] {return false;}
  }
  return false;   // the same event
}
// The poll's result over the responses in set: one counted response per
// pubkey, the newest before the end.
n88_tally :: (poll:NostrEvent,set:[]NostrEvent)->N88Tally {
  t:N88Tally;p:=n88_poll(poll);
  if p.status!=N88_OK {t.status=p.status;return t;}
  for i:0..<set.count {
    if set[i].kind!=N88_KIND_RESPONSE {continue;}
    r:=n88_response(set[i]);
    if r.status!=N88_OK {t.refused+=1;continue;}
    if !ns_equal32(r.poll,poll.id) {continue;}
    if p.has_end && set[i].created_at>p.ends_at {t.late+=1;continue;}
    repeat:=false;
    for k:0..<i {if ns_equal32(set[k].id,set[i].id) {repeat=true;}}
    if repeat {t.repeated+=1;continue;}
    replaced:=false;
    for k:0..<set.count {
      if k!=i && ns_equal32(set[k].public_key,set[i].public_key) && n88_counts(set[k],poll.id,p) && n88_newer(set[k],set[i]) {replaced=true;}
    }
    if replaced {t.superseded+=1;continue;}
    t.voters+=1;
    chosen:[N88_MAX_OPTIONS]bool;any:=false;
    for c:0..<r.n {
      x:=r.choices[c];k:=n88_index(p,cast(string)x.b[0..<x.n]);
      if k<0 {t.ignored+=1;continue;}
      if chosen[k] || (p.single && any) {continue;}
      chosen[k]=true;any=true;t.counts[k]+=1;
    }
    if !any {t.blank+=1;}
  }
  return t;
}
```

The tally keeps no table of pubkeys. For each response, it looks through
the whole set for a newer counted response by the same pubkey. That is
O(n²) in the size of the set and needs no memory beyond the record.

### Worked example: four responses, one repeat voter, one late vote

Alice's poll P asks "Pineapple on pizza" with options `qj518h583` (Yay)
and `gga6cdnqj` (Nay), singlechoice, `endsAt` T0 + 3600 (1761690000).
Four responses name P:

| Response | Voter | Time | Response tags |
| --- | --- | --- | --- |
| V1 | Bob | T0 + 60 | `qj518h583` |
| V2 | Carol | T0 + 120 | `zzz`, `qj518h583` |
| V3 | Bob | T0 + 300 | `gga6cdnqj` |
| V4 | Carol | T0 + 7200 | `gga6cdnqj` |

1. V4 is at T0 + 7200, after `endsAt`: `late` becomes 1. It is not counted
   and replaces nothing.
2. V1 is Bob's. V3 is also Bob's, counts, and is newer (T0 + 300 > T0 + 60):
   V1 is superseded. `superseded` becomes 1.
3. V2 is Carol's only counted response. `voters` becomes 1. Its first id,
   `zzz`, is not an option: `ignored` becomes 1. Its next id is Yay, the
   first valid one: Yay becomes 1.
4. V3 is Bob's newest counted response. `voters` becomes 2. Nay becomes 1.

The result: Yay 1, Nay 1, voters 2, blank 0, superseded 1, late 1, ignored
1. If V4 had counted, it would have replaced Carol's Yay and the result
would be Yay 0, Nay 2. If the tally kept every response, Bob would have
voted twice.

## Highlights

A highlight (9802) has the highlighted text as its content. NIP-84 says it
SHOULD tag its source: `e` or `a` for a Nostr event, `i` for a NIP-73
identifier and `r` for anything else. This reader counts the sources and
keeps the first. A highlight with no source still reads, with source
`N84_NONE`, because the NIP does not require one.

`p` tags credit authors, with a role as the last value. In a quote
highlight, which has a `comment` tag, a `p` tag with role `mention` and an
`r` tag marked `mention` belong to the comment, not to the highlighted
work. The reader counts those as mentions.

<!-- source: coax/lib/nostr/nip84.coax record -->
```jai
N84Highlight :: struct {
  status:int=N84_EVENT; at:int=-1; tag_error:int;
  source:int=N84_NONE; first:N32Text;   // the first source's tag and value
  sources:int;                          // e, a, i and source r tags
  authors:int;                          // p tags with any role but mention
  mentions:int;                         // p and r tags marked mention
  has_context:bool; context:N32Text;
  has_comment:bool; comment:N32Text;
}
```

`n32_tag` decodes only a tag's first two values. A `p` tag's role is its
fourth, so `n84_last` reads a tag's last value.

<!-- source: coax/lib/nostr/nip84.coax last -->
```jai
// The last value of the index'th tag, decoded, or empty when the tag is
// missing, malformed or the value too long.
n84_last :: (e:NostrEvent,index:int)->N32Text {
  r:N32Text;none:N32Text;j:[1]Json;if index<0 || !nt_start(j[..],e) {return none;}
  k:=0;tok:=json_next(j[..]);
  while tok==JSON_ARRAY && k<index {skipped:=json_skip(j[..]);k+=1;tok=json_next(j[..]);}
  if tok!=JSON_ARRAY {return none;}
  v:=json_next(j[..]);
  while v==JSON_STRING {
    n:=json_unescape(json_text(j[..]),r.b[..]);
    if n<0 {return none;}
    r.n=n;v=json_next(j[..]);
  }
  if v!=JSON_ARRAY_END {return none;}
  return r;
}
```

<!-- source: coax/lib/nostr/nip84.coax read -->
```jai
// A highlight: its source, the authors it credits, its context and comment.
n84_read :: (e:NostrEvent)->N84Highlight {
  h:N84Highlight;if e.error!=NE_OK {return h;}
  if e.kind!=N84_KIND_HIGHLIGHT {return n84_refuse(h,N84_KIND,-1);}
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n84_refuse(h,N84_LIMIT,i);}
    if t.error!=NT_OK {return n84_refuse(h,N84_EVENT,i);}
    tag:=t.v[0];kind:=N84_NONE;mention:=false;
    if n32_is(tag,"e") || n32_is(tag,"a") || n32_is(tag,"p") {
      s:=nostr_tag(e,i);
      if s.error!=NT_OK {h.tag_error=s.error;return n84_refuse(h,N84_TAG,i);}
      if s.type==NT_E {kind=N84_E;} else if s.type==NT_A {kind=N84_A;}
      else {
        mention=t.values>=4 && n32_is(n84_last(e,i),"mention");
        if !mention {h.authors+=1;}
      }
    } else if n32_is(tag,"i") || n32_is(tag,"r") {
      if t.v[1].n==0 {return n84_refuse(h,N84_SHAPE,i);}
      mention=n32_is(tag,"r") && n32_is(t.v[2],"mention");
      if !mention {kind=ifx n32_is(tag,"i") then N84_I else N84_R;}
    } else if n32_is(tag,"context") && !h.has_context {h.has_context=true;h.context=t.v[1];}
    else if n32_is(tag,"comment") && !h.has_comment {h.has_comment=true;h.comment=t.v[1];}
    if mention {h.mentions+=1;}
    if kind!=N84_NONE {
      if h.source==N84_NONE {h.source=kind;h.first=t.v[1];}
      h.sources+=1;
    }
    i+=1;
  }
  h.status=N84_OK;return h;
}
```

### Not built

- Recurring calendar events: NIP-52 leaves them to clients, and so does
  this chapter.
- Fetching the event an RSVP or a calendar points at, or checking that an
  RSVP's `p` tag is that event's author.
- Checking a time zone name against the IANA database, or converting times
  between zones.
- Sending responses to the poll's `relay` tags, rejecting responses dated
  before the poll, and the follow-set or web-of-trust curation NIP-88
  suggests.
- Checking that a highlight's text appears in its source.
- Writers for calendar events, calendars, polls and highlights. The
  example writes their tags as literal JSON.

## The example

The example is
[nostr/examples/calendars-polls.coax](../examples/calendars-polls.coax). It
signs 11 events with published test keys: BIP-340 test vectors 0 to 3 as
Alice, Bob, Carol and Mallory, and the secret keys 1 and 2 as Dave and
Erin. Alice publishes a date event M (a two-day meetup), a time event T (a
call), a calendar C that lists both, and the poll P. Bob and Carol RSVP to
M: Bob's tags come from `n52_rsvp_tags`, and Carol's carry an `fb` on a
declined answer. Bob and Carol send the four responses of the worked
example, written by `n88_response_tags`. Erin highlights a sentence from a
web page. Then the example reads every event back and tallies the poll.
The refusals use unsigned events, because no signer here would write them.

<!-- source: nostr/examples/calendars-polls.coax example -->
```jai
#load "../../coax/lib/nostr/nip52.coax";
#load "../../coax/lib/nostr/nip88.coax";
#load "../../coax/lib/nostr/nip84.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

CALENDAR :: [14] string.["ok", "event", "kind", "bad tag", "missing", "bad date", "bad time", "end not after start", "bad time zone", "bad day", "not a calendar event", "unknown status", "unknown fb", "limit"];
POLL :: [10] string.["ok", "event", "kind", "bad option id", "repeated option id", "unknown poll type", "bad endsAt", "missing", "bad e tag", "limit"];
HIGHLIGHT :: [6] string.["ok", "event", "kind", "bad tag", "empty value", "limit"];
SOURCE :: [5] string.["none", "e", "a", "i", "r"];
// BIP-340 test vectors 0 to 3's public keys, then the public keys of the
// secret keys 1 and 2.
PEOPLE :: [6] string.["Alice", "Bob", "Carol", "Mallory", "Dave", "Erin"];
PUB :: [6] string.[
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
  "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8",
  "25d1dff95105f5253c4022f628a996ad3a0d95fbf21d468a1b33f8c160d8f517",
  "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
  "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"];
// Their secret keys: published test values. Never sign with a real key in
// an example.
SECRET :: [6] string.[
  "0000000000000000000000000000000000000000000000000000000000000003",
  "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef",
  "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9",
  "0b432b2677937381aef05bb02a66ecd012773062cf3fa2549e44f58ed2401710",
  "0000000000000000000000000000000000000000000000000000000000000001",
  "0000000000000000000000000000000000000000000000000000000000000002"];
ALICE :: 0; BOB :: 1; CAROL :: 2; MALLORY :: 3; DAVE :: 4; ERIN :: 5;
T0 :: 1761686400;
MEETUP :: "31922:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:meetup";

// Decode hex that the program itself supplies; it must be valid.
decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}
hex32 :: (id: [32] u8, out: [] u8) -> string {
  own := id; r := hex_encode_checked(cast(string) own[..], out, false);
  #assert r.error == NC_OK;
  return cast(string) out[0..<64];
}
key_of :: (who: int) -> [32] u8 { k: [32] u8; decode_hex(PUB[who], k[..]); return k; }
show_key :: (key: [32] u8) {
  for k: 0..<PEOPLE.count { if ns_equal32(key_of(k), key) { print("%", PEOPLE[k]); return; } }
  text: [64] u8; print("%...", hex32(key, text[..])[0..<8]);
}
say :: (t: N32Text) { own := t; print("%", cast(string) own.b[0..<own.n]); }

// Sign an event into out and read it back with its ID and signature checked.
sign :: (who: int, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  key: [32] u8; decode_hex(SECRET[who], key[..]);
  aux: [32] u8; work: [1] NostrSignWork; scratch: [8192] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  #assert n > 0;
  check: [8192] u8;
  e := nostr_event_verify_vartime(cast(string) out[0..<n], check[..], false);
  #assert e.error == NE_OK && e.verified;
  return e;
}

// The events: who signs, when (after T0), the kind, the tags and the
// content. Tags written by the library's writers are empty here and filled
// in by tags_of.
EVENTS :: 11;
NAME :: [EVENTS] string.["M", "T", "C", "R1", "R2", "P", "V1", "V2", "V3", "V4", "H"];
WHO :: [EVENTS] int.[ALICE, ALICE, ALICE, BOB, CAROL, ALICE, BOB, CAROL, BOB, CAROL, ERIN];
AT :: [EVENTS] int.[0, 10, 20, 30, 40, 50, 60, 120, 300, 7200, 100];
KIND :: [EVENTS] int.[31922, 31923, 31924, 31925, 31925, 1068, 1018, 1018, 1018, 1018, 9802];
TAGS :: [EVENTS] string.[
  "[[\"d\",\"meetup\"],[\"title\",\"Nostr meetup\"],[\"start\",\"2026-11-20\"],[\"end\",\"2026-11-22\"],[\"location\",\"Austin\"],[\"p\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\",\"\",\"speaker\"]]",
  "[[\"d\",\"call\"],[\"title\",\"Planning call\"],[\"start\",\"1795280400\"],[\"end\",\"1795291200\"],[\"D\",\"20778\"],[\"start_tzid\",\"America/Chicago\"]]",
  "[[\"d\",\"meetups\"],[\"title\",\"Meetups\"],[\"a\",\"31922:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:meetup\"],[\"a\",\"31923:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:call\"]]",
  "",
  "[[\"a\",\"31922:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:meetup\"],[\"d\",\"rsvp-meetup\"],[\"status\",\"declined\"],[\"fb\",\"free\"]]",
  "[[\"option\",\"qj518h583\",\"Yay\"],[\"option\",\"gga6cdnqj\",\"Nay\"],[\"relay\",\"wss://relay.example.com\"],[\"polltype\",\"singlechoice\"],[\"endsAt\",\"1761690000\"]]",
  "", "", "", "",
  "[[\"r\",\"https://example.com/protocol\",\"source\"],[\"p\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"\",\"author\"],[\"context\",\"Nostr is the simplest open protocol that works. It does not rely on any trusted central server.\"]]"];
CONTENT :: [EVENTS] string.["Two days of talks.", "", "", "", "Out of town.", "Pineapple on pizza", "", "", "", "", "Nostr is the simplest open protocol that works."];
// The responses' choices: V1 Yay; V2 an unknown id, then Yay; V3 Nay; V4 Nay.
CHOICE :: [8] string.["qj518h583", "zzz", "qj518h583", "gga6cdnqj", "gga6cdnqj", "", "", ""];
CHOICE_AT :: [4] int.[0, 1, 3, 4];
CHOICE_N :: [4] int.[1, 2, 1, 1];

// The tags of event k: its template, or the RSVP and response writers'
// output, which needs the poll's id.
tags_of :: (k: int, poll: NostrEvent, out: [] u8) -> string {
  n := 0;
  if k == 3 { n = n52_rsvp_tags(MEETUP, "rsvp-meetup", N52_ACCEPTED, N52_BUSY, out); }
  else if k >= 6 && k <= 9 {
    pick: [2] string; at := CHOICE_AT[k - 6];
    for c: 0..<CHOICE_N[k - 6] { pick[c] = CHOICE[at + c]; }
    n = n88_response_tags(poll.id, pick[0..<CHOICE_N[k - 6]], out);
  } else { n = json_raw(out, 0, TAGS[k]); }
  #assert n > 0;
  return cast(string) out[0..<n];
}

show_event :: (label: string, e: NostrEvent) {
  r := n52_read(e);
  print("%: %", label, CALENDAR[r.status]);
  if r.at >= 0 { print(" at tag %", r.at); }
  if r.status == N52_MISSING { print(" %", N52_NEEDS[r.missing]); }
  if r.status != N52_OK { print("\n"); return; }
  print(", kind %, \"", r.kind); say(r.title); print("\", ");
  if r.kind == N52_DATE_KIND { print("days % to %", r.start, r.end); }
  else {
    print("seconds % to %, ", r.start, r.end); say(r.start_tzid); print(" to "); say(r.end_tzid);
    print(", D tags %", r.days);
  }
  print(", participants %, by ", r.participants); show_key(e.public_key); print("\n");
}
show_ref :: (x: N52Ref) { print("% ", x.kind); say(x.d); print(" by "); show_key(x.author); }
show_rsvp :: (label: string, e: NostrEvent) {
  r := n52_rsvp(e);
  print("%: %", label, CALENDAR[r.status]);
  if r.at >= 0 { print(" at tag %", r.at); }
  if r.status != N52_OK { print("\n"); return; }
  print(", %, fb ", N52_STATUSES[r.answer]);
  if r.fb >= 0 { print("%", N52_FBS[r.fb]); } else { print("absent"); }
  print(", to "); show_ref(r.to); print(", by "); show_key(e.public_key); print("\n");
}
show_poll :: (label: string, e: NostrEvent) {
  p := n88_poll(e);
  print("%: %", label, POLL[p.status]);
  if p.at >= 0 { print(" at tag %", p.at); }
  if p.status != N88_OK { print("\n"); return; }
  print(", % choice", ifx p.single then "single" else "multiple");
  for k: 0..<p.n { print(", "); say(p.options[k].id); print(" "); say(p.options[k].label); }
  if p.has_end { print(", ends at %", p.ends_at); }
  print("\n");
}
show_response :: (label: string, e: NostrEvent, poll: NostrEvent) {
  r := n88_response(e);
  print("%: %", label, POLL[r.status]);
  if r.at >= 0 { print(" at tag %", r.at); }
  if r.status != N88_OK { print("\n"); return; }
  print(", by "); show_key(e.public_key); print(" at T0+%, chooses", e.created_at - T0);
  for k: 0..<r.n { print(" "); say(r.choices[k]); }
  print(", to %\n", ifx ns_equal32(r.poll, poll.id) then "P" else "another poll");
}
show_highlight :: (label: string, e: NostrEvent) {
  h := n84_read(e);
  print("%: %", label, HIGHLIGHT[h.status]);
  if h.at >= 0 { print(" at tag %", h.at); }
  if h.status != N84_OK { print("\n"); return; }
  print(", source % ", SOURCE[h.source]); say(h.first);
  print(", sources %, authors %, mentions %, context %, by ", h.sources, h.authors, h.mentions, h.has_context); show_key(e.public_key); print("\n");
}

// An unsigned event by Mallory with these tags, parsed but not verified:
// its id and sig are zeros. Used for the refusals, which no signer here
// would write.
unsigned :: (kind: int, tags: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"");
  n = json_raw(out, n, PUB[MALLORY]); n = json_raw(out, n, "\",\"created_at\":1761686400,\"kind\":");
  n = json_write_int(out, n, kind); n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":\"\",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}

main :: () {
  all: [EVENTS] NostrEvent; buf: [EVENTS][8192] u8; tags: [4096] u8;
  for k: 0..<EVENTS {
    all[k] = sign(WHO[k], T0 + AT[k], KIND[k], tags_of(k, all[5], tags[..]), CONTENT[k], buf[k][..]);
  }
  print("each event's id\n");
  for k: 0..<EVENTS { hex: [64] u8; print("%: %, verified %\n", NAME[k], hex32(all[k].id, hex[..]), all[k].verified); }

  print("calendar events\n");
  show_event("M", all[0]); show_event("T", all[1]);
  c := n52_calendar(all[2]);
  print("C: %, \"", CALENDAR[c.status]); say(c.title); print("\", % events:", c.n);
  for k: 0..<c.n { print(" "); show_ref(c.events[k]); print(";"); }
  print("\n");
  print("RSVPs\n");
  show_rsvp("R1", all[3]); show_rsvp("R2", all[4]);

  print("the poll and its responses\n");
  show_poll("P", all[5]);
  for k: 6..<10 { show_response(NAME[k], all[k], all[5]); }
  t := n88_tally(all[5], all[..]);
  p := n88_poll(all[5]);
  print("tally:");
  for k: 0..<p.n { print(" "); say(p.options[k].label); print(" %,", t.counts[k]); }
  print(" voters %, blank %, superseded %, late %, ignored %, repeated %, refused %\n", t.voters, t.blank, t.superseded, t.late, t.ignored, t.repeated, t.refused);

  print("highlight\n");
  show_highlight("H", all[10]);

  print("refusals\n");
  bad: [4096] u8;
  show_event("date event ending before it starts", unsigned(31922, "[[\"d\",\"m\"],[\"title\",\"T\"],[\"start\",\"2026-11-22\"],[\"end\",\"2026-11-20\"]]", bad[..]));
  show_event("date event on 2026-02-30", unsigned(31922, "[[\"d\",\"m\"],[\"title\",\"T\"],[\"start\",\"2026-02-30\"]]", bad[..]));
  show_event("time event ending as it starts", unsigned(31923, "[[\"d\",\"c\"],[\"title\",\"T\"],[\"start\",\"1795280400\"],[\"end\",\"1795280400\"],[\"D\",\"20778\"]]", bad[..]));
  show_event("time event without a D tag", unsigned(31923, "[[\"d\",\"c\"],[\"title\",\"T\"],[\"start\",\"1795280400\"]]", bad[..]));
  show_rsvp("RSVP with status maybe", unsigned(31925, "[[\"a\",\"31922:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:meetup\"],[\"d\",\"r\"],[\"status\",\"maybe\"]]", bad[..]));
  show_poll("poll of type ranked", unsigned(1068, "[[\"option\",\"a\",\"A\"],[\"option\",\"b\",\"B\"],[\"polltype\",\"ranked\"]]", bad[..]));
  show_poll("poll with a repeated option id", unsigned(1068, "[[\"option\",\"a\",\"A\"],[\"option\",\"a\",\"B\"]]", bad[..]));
  show_response("response without an e tag", unsigned(1018, "[[\"response\",\"qj518h583\"]]", bad[..]), all[5]);
  show_highlight("highlight of a short event id", unsigned(9802, "[[\"e\",\"5c83da77\"]]", bad[..]));
}
```

Run it with:

```shell
coax/bin/coax run nostr/examples/calendars-polls.coax --run-steps 250000000
```

It prints this transcript, which `nostr/fixtures/calendars-polls.out` holds
and the book's checks compare in debug and release. It exits with status 0.

```text
each event's id
M: 35537e21cc60eff63bfc8172fa9275886b44a91400a773f0f2ca8f2fd20c8588, verified true
T: a3af333a2ceac728c1b3040956fee0a5985f0130eebf258fb50cf384472aa59c, verified true
C: 038e8315eb5b69d79cac822ac96c35625596ea129c64ca8e6efa3fc504d2ff10, verified true
R1: 20469982e6bda3e424e1722fd059533cee76e907f696f71f15636c21b74ca30a, verified true
R2: 38e4f7ba79c12ebc1cb0b4e72e775f9fabbdd66413b9cf760355a7624bd92647, verified true
P: 84d2f2d793dc2d1b63c07b8d530cd70475d7643755fcbd16917affbb1fe2995f, verified true
V1: e627ce2ca962ac1d905b784ee56bbd80bbe0b55299eca2bd44c1dcfcc9cb920e, verified true
V2: bcffcedc092e6c6df827a25d9f78e2b597835b936dd186689b38aefacd7c36bc, verified true
V3: 994ba18c847bcbde267a80eee88a948d1ccae9e2a1756e656a790014e006e8b2, verified true
V4: e574ec313ae7284bb8180998207aa237f078efffd1bb0e24264a8ba32f446204, verified true
H: 8cfe87ca81c18c1532a4ab7024b555b41ae9f93279cec13029b9135e337e4cb5, verified true
calendar events
M: ok, kind 31922, "Nostr meetup", days 20777 to 20779, participants 1, by Alice
T: ok, kind 31923, "Planning call", seconds 1795280400 to 1795291200, America/Chicago to America/Chicago, D tags 1, participants 0, by Alice
C: ok, "Meetups", 2 events: 31922 meetup by Alice; 31923 call by Alice;
RSVPs
R1: ok, accepted, fb busy, to 31922 meetup by Alice, by Bob
R2: ok, declined, fb absent, to 31922 meetup by Alice, by Carol
the poll and its responses
P: ok, single choice, qj518h583 Yay, gga6cdnqj Nay, ends at 1761690000
V1: ok, by Bob at T0+60, chooses qj518h583, to P
V2: ok, by Carol at T0+120, chooses zzz qj518h583, to P
V3: ok, by Bob at T0+300, chooses gga6cdnqj, to P
V4: ok, by Carol at T0+7200, chooses gga6cdnqj, to P
tally: Yay 1, Nay 1, voters 2, blank 0, superseded 1, late 1, ignored 1, repeated 0, refused 0
highlight
H: ok, source r https://example.com/protocol, sources 1, authors 1, mentions 0, context true, by Erin
refusals
date event ending before it starts: end not after start at tag 3
date event on 2026-02-30: bad date at tag 2
time event ending as it starts: end not after start at tag 3
time event without a D tag: missing D
RSVP with status maybe: unknown status at tag 2
poll of type ranked: unknown poll type at tag 2
poll with a repeated option id: repeated option id at tag 1
response without an e tag: missing
highlight of a short event id: bad tag at tag 0
```

The days of M and the tally match the worked examples. R2's `fb` reads as
absent because Carol declined. The refusals show each broken input and its
exact diagnostic:

- `date event ending before it starts`: `end` 2026-11-20 is before
  `start` 2026-11-22, at the `end` tag, 3.
- `date event on 2026-02-30`: February 2026 has 28 days, at the `start`
  tag, 2.
- `time event ending as it starts`: `end` equals `start`; `end` is
  exclusive, so the event would be empty.
- `time event without a D tag`: kind 31923 needs a day tag.
- `RSVP with status maybe`: not one of the three answers, at tag 2.
- `poll of type ranked`: not `singlechoice` or `multiplechoice`, at tag 2.
- `poll with a repeated option id`: two options with id `a`, at tag 1.
- `response without an e tag`: the response names no poll.
- `highlight of a short event id`: the `e` tag's id is 8 hex digits, at
  tag 0.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/calendars-polls.coax](../examples/checks/calendars-polls.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- The ids of all 11 events, from a Python script written from NIP-01,
  NIP-52, NIP-88 and NIP-84 (`hashlib.sha256` over NIP-01's
  serialization), with the RSVP's and the responses' tags written by hand
  in Python. The checks read each event with Python's id and require that
  the recomputed id matches.
- The poll tally by hand: Yay 1, Nay 1. The same Python script tallies the
  responses from NIP-88's text and gets Yay 1, Nay 1. The checks also
  tally in reverse order, without V3 (Yay 2), without V2, and with V4
  twice.
- The day numbers 20777 and 20779 by hand and from Python's `datetime`,
  and the call's day 20778.
- NIP-88's example response as published. Its id does not match its
  content: Python computes `b75ed405…` for it, and Coax computes the same
  and refuses the event. Its tags still read, and against the NIP's
  example poll it counts one vote for Nay, with `m3agjsdq1` ignored.
- NIP-52's time-event and RSVP examples and NIP-84's attribution example,
  with their placeholders filled, each read as the NIP describes.

<!-- source: nostr/examples/checks/calendars-polls.coax check -->
```jai
// The ids from a Python script written from NIP-01, NIP-52, NIP-88 and
// NIP-84 (it shares no code with Coax): hashlib.sha256 of
// json.dumps([0, pubkey, created_at, kind, tags, content],
// separators=(",", ":"), ensure_ascii=False), over the same events, with
// the RSVP's and the responses' tags written by hand in Python.
PY_ID :: [EVENTS] string.[
  "35537e21cc60eff63bfc8172fa9275886b44a91400a773f0f2ca8f2fd20c8588",
  "a3af333a2ceac728c1b3040956fee0a5985f0130eebf258fb50cf384472aa59c",
  "038e8315eb5b69d79cac822ac96c35625596ea129c64ca8e6efa3fc504d2ff10",
  "20469982e6bda3e424e1722fd059533cee76e907f696f71f15636c21b74ca30a",
  "38e4f7ba79c12ebc1cb0b4e72e775f9fabbdd66413b9cf760355a7624bd92647",
  "84d2f2d793dc2d1b63c07b8d530cd70475d7643755fcbd16917affbb1fe2995f",
  "e627ce2ca962ac1d905b784ee56bbd80bbe0b55299eca2bd44c1dcfcc9cb920e",
  "bcffcedc092e6c6df827a25d9f78e2b597835b936dd186689b38aefacd7c36bc",
  "994ba18c847bcbde267a80eee88a948d1ccae9e2a1756e656a790014e006e8b2",
  "e574ec313ae7284bb8180998207aa237f078efffd1bb0e24264a8ba32f446204",
  "8cfe87ca81c18c1532a4ab7024b555b41ae9f93279cec13029b9135e337e4cb5"];
SIG :: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

// Event k with Python's id and a zero signature, read with its id
// recomputed; the signature is not checked.
py :: (k: int, poll: NostrEvent, out: [] u8, scratch: [] u8) -> NostrEvent {
  tags: [1024] u8;
  n := json_raw(out, 0, "{\"id\":\""); n = json_raw(out, n, PY_ID[k]);
  n = json_raw(out, n, "\",\"pubkey\":\""); n = json_raw(out, n, PUB[WHO[k]]);
  n = json_raw(out, n, "\",\"created_at\":"); n = json_write_int(out, n, T0 + AT[k]);
  n = json_raw(out, n, ",\"kind\":"); n = json_write_int(out, n, KIND[k]);
  n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags_of(k, poll, tags[..]));
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, CONTENT[k]);
  n = json_raw(out, n, ",\"sig\":\""); n = json_raw(out, n, SIG); n = json_raw(out, n, "\"}");
  #assert n > 0;
  return nostr_event_read(cast(string) out[0..<n], scratch, false);
}
read_all :: (ev: [] NostrEvent, buf: [][2048] u8) {
  scratch: [4096] u8; none: NostrEvent;
  ev[5] = py(5, none, buf[5][..], scratch[..]);   // the poll first: responses name it
  for k: 0..<EVENTS { if k != 5 { ev[k] = py(k, ev[5], buf[k][..], scratch[..]); } }
}

#test "the ids are Python's" {
  buf: [EVENTS][2048] u8; ev: [EVENTS] NostrEvent; read_all(ev[..], buf[..]);
  for k: 0..<EVENTS { #assert ev[k].error == NE_OK && ns_equal32(ev[k].computed_id, ev[k].id); }
}

// The tally by hand. P ends at T0+3600. V4 (Carol, T0+7200) is late and
// is dropped first. Bob has V1 (T0+60, Yay) and V3 (T0+300, Nay): V3 is
// newer, V1 is superseded. Carol has V2 (T0+120): "zzz" is no option and
// is ignored, then Yay. Yay 1 (Carol), Nay 1 (Bob). Python's tally, written
// from NIP-88's text, gives the same.
#test "the poll tally matches the hand count, in any order" {
  buf: [EVENTS][2048] u8; ev: [EVENTS] NostrEvent; read_all(ev[..], buf[..]);
  t := n88_tally(ev[5], ev[..]);
  #assert t.status == N88_OK && t.counts[0] == 1 && t.counts[1] == 1 && t.voters == 2;
  #assert t.superseded == 1 && t.late == 1 && t.ignored == 1 && t.blank == 0 && t.repeated == 0 && t.refused == 0;
  rev: [EVENTS] NostrEvent; for k: 0..<EVENTS { rev[k] = ev[EVENTS - 1 - k]; }
  t = n88_tally(ev[5], rev[..]); #assert t.counts[0] == 1 && t.counts[1] == 1 && t.superseded == 1 && t.late == 1;
  // Without V3, Bob's Yay stands: Yay 2. Without V2, only Bob votes.
  some: [3] NostrEvent; some[0] = ev[6]; some[1] = ev[7]; some[2] = ev[9];
  t = n88_tally(ev[5], some[..]); #assert t.counts[0] == 2 && t.counts[1] == 0 && t.late == 1 && t.superseded == 0;
  some[1] = ev[8]; t = n88_tally(ev[5], some[..]); #assert t.counts[0] == 0 && t.counts[1] == 1 && t.voters == 1;
  // V4 twice is still one late vote each time it appears, never counted.
  some[0] = ev[9]; some[1] = ev[9]; some[2] = ev[7];
  t = n88_tally(ev[5], some[..]); #assert t.counts[0] == 1 && t.late == 2 && t.voters == 1;
}

// Dates by hand and by Python's datetime: 2026-11-20 is 20777 days after
// 1970-01-01, 2026-11-22 is 20779. 1795280400 / 86400 = 20778.7, so the
// call's day is 20778.
#test "the calendar readings match the hand values" {
  buf: [EVENTS][2048] u8; ev: [EVENTS] NostrEvent; read_all(ev[..], buf[..]);
  m := n52_read(ev[0]); #assert m.status == N52_OK && m.start == 20777 && m.end == 20779 && m.end - m.start == 2;
  c := n52_read(ev[1]); #assert c.status == N52_OK && c.end - c.start == 10800 && c.start / 86400 == 20778;
  l := n52_calendar(ev[2]); #assert l.status == N52_OK && l.n == 2 && ns_equal32(l.events[0].author, key_of(ALICE));
  r := n52_rsvp(ev[3]); #assert r.status == N52_OK && r.answer == N52_ACCEPTED && r.fb == N52_BUSY && n32_is(r.to.d, "meetup");
  r = n52_rsvp(ev[4]); #assert r.status == N52_OK && r.answer == N52_DECLINED && r.fb == -1;
  h := n84_read(ev[10]); #assert h.status == N84_OK && h.source == N84_R && h.authors == 1;
}

// NIP-88's example response, as published. Its id does not match its
// content: Python computes b75ed405... for it, and so does Coax, which
// refuses the event (NE_ID). Its tags still read as the NIP describes.
#test "NIP-88's published response: the id doesn't recompute, the tags read" {
  text := "{\"content\":\"\",\"created_at\":1720097117,\"id\":\"60a005e32e9596c3f544a841a9bc4e46d3020ca3650d6a739c95c1568e33f6d8\",\"kind\":1018,\"pubkey\":\"1bc70a0148b3f316da33fe7e89f23e3e71ac4ff998027ec712b905cd24f6a411\",\"sig\":\"30071a633c65db8f3a075c7a8de757fbd8ce65e3607f4ba287fe6d7fbf839a380f94ff4e826fbba593f6faaa13683b7ea9114ade140720ecf4927010ebf3e44f\",\"tags\":[[\"e\",\"1fc80cf813f1af33d5a435862b7ef7fb96b47e68a48f1abcadf8081f5a545550\"],[\"response\",\"gga6cdnqj\"],[\"response\",\"m3agjsdq1\"]]}";
  scratch: [4096] u8; e := nostr_event_read(text, scratch[..], false);
  python: [32] u8; decode_hex("b75ed405a7b6a5627234c69b321f6c418dbc28ba8a0a2a9d208c09a52790330b", python[..]);
  #assert e.error == NE_ID && ns_equal32(e.computed_id, python);
  p := ne_parse(text); r := n88_response(p);
  #assert r.status == N88_OK && r.n == 2 && n32_is(r.choices[0], "gga6cdnqj") && r.poll[0] == 0x1f;
  // Against the NIP's example poll, singlechoice: Nay; m3agjsdq1 ignored.
  b: [1024] u8;
  poll := unsigned(1068, "[[\"option\",\"qj518h583\",\"Yay\"],[\"option\",\"gga6cdnqj\",\"Nay\"],[\"polltype\",\"singlechoice\"]]", b[..]);
  decode_hex("1fc80cf813f1af33d5a435862b7ef7fb96b47e68a48f1abcadf8081f5a545550", poll.id[..]);
  set: [1] NostrEvent; set[0] = p;
  t := n88_tally(poll, set[..]); #assert t.counts[1] == 1 && t.counts[0] == 0 && t.ignored == 1;
}

// NIP-52's and NIP-84's examples, placeholders filled in.
#test "NIP-52's and NIP-84's examples read as the NIPs say" {
  b: [2048] u8;
  e := n52_read(unsigned(31923, "[[\"d\",\"x\"],[\"title\",\"t\"],[\"summary\",\"s\"],[\"image\",\"https://i\"],[\"start\",\"7132237200\"],[\"end\",\"7132240800\"],[\"D\",\"82549\"],[\"start_tzid\",\"America/Costa_Rica\"],[\"end_tzid\",\"America/Costa_Rica\"],[\"location\",\"l\"],[\"g\",\"9q8yy\"],[\"p\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"wss://relay.example.com\",\"host\"]]", b[..]));
  #assert e.status == N52_OK && e.start / 86400 == 82549 && e.participants == 1 && n32_is(e.end_tzid, "America/Costa_Rica");
  r := n52_rsvp(unsigned(31925, "[[\"e\",\"35537e21cc60eff63bfc8172fa9275886b44a91400a773f0f2ca8f2fd20c8588\",\"wss://relay.example.com\"],[\"a\",\"31922:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:meetup\",\"wss://relay.example.com\"],[\"d\",\"x\"],[\"status\",\"tentative\"],[\"fb\",\"free\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\",\"wss://relay.example.com\"]]", b[..]));
  #assert r.status == N52_OK && r.answer == N52_TENTATIVE && r.fb == N52_FREE && r.has_revision && r.has_author;
  h := n84_read(unsigned(9802, "[[\"p\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"wss://relay.example.com\",\"author\"],[\"p\",\"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5\",\"wss://relay.example.com\",\"author\"],[\"p\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\",\"wss://relay.example.com\",\"editor\"]]", b[..]));
  #assert h.status == N84_OK && h.authors == 3 && h.source == N84_NONE;
}
```

### The library's own checks

`nip52.coax`, `nip88.coax` and `nip84.coax` have their own `#test` blocks:
every month's last day across a leap year, every refusal and the tag index
it reports, the first-of-each-tag rules, every bound at its edge, forged tag
arrays, the writers' exact bytes and their size limits, and the tally's
ties, rivals at exactly `endsAt`, repeated events, blank votes and ids that
differ only in their first or last byte.

```shell
coax/bin/coax test coax/lib/nostr/nip52.coax
coax/bin/coax test coax/lib/nostr/nip88.coax
coax/bin/coax test coax/lib/nostr/nip84.coax
coax/bin/coax test nostr/examples/checks/calendars-polls.coax --run-steps 250000000
coax/bin/coax mutate coax/lib/nostr/nip52.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip88.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip84.coax --root-only
```

On 2026-11-01, `coax test` passed all 211, 209 and 206 checks of the three
files (with the loaded files'), and all 376 of the example's checks.
`coax mutate --root-only` killed all 590 mutants of `nip52.coax`, all 280
of `nip88.coax` and all 128 of `nip84.coax`, with none surviving and none
marked equivalent. The full mutation run (`COAX_MUTATION_FULL=1`) and a
mutation run of the example were not run.

## Summary

- `n52_date` accepts only real `YYYY-MM-DD` dates and turns them into day
  numbers, so `n52_read` checks `start < end` the same way for dates and
  for seconds.
- `n52_calendar` and `n52_rsvp` check that their `a` tags point at the
  right kinds. An RSVP's `fb` is ignored when it declines.
- `n88_tally` counts one response per pubkey, the newest at or before
  `endsAt`, with ties broken by the lower id. It ignores option ids the
  poll doesn't define, and a singlechoice poll counts the first valid one.
- `n84_read` finds a highlight's source and separates credited authors
  from the mentions in a quote highlight's comment.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Sources

- [NIP-52](../nips/official/52.md)
- [NIP-88](../nips/official/88.md)
- [NIP-84](../nips/official/84.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
