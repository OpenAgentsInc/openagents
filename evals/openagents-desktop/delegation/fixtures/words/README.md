# words

`split(line)` in `words.py` splits a command line into its words, as a POSIX shell reads them.

- Words are separated by runs of whitespace: space, tab, newline and carriage return.
- Single quotes keep everything up to the next single quote as it is. Nothing is special inside.
- Double quotes keep everything up to the next unescaped double quote. Inside them a backslash
  is special only before `"` or `\`: the pair becomes that one character. Before any other
  character the backslash stays, and so does the character.
- Outside quotes a backslash makes the next character an ordinary one, whatever it is, and the
  backslash goes.
- Quoted and unquoted parts that touch are one word: `a"b c"'d'` is the word `ab cd`. Empty
  quotes make an empty word: `a '' b` is three words.
- Every other character is ordinary, `#`, `$`, `*` and `=` included.
- A quote that is never closed, or a backslash at the very end outside single quotes, raises
  `ValueError`.

`words.py` must do the splitting itself. It must not use the `shlex` module.
