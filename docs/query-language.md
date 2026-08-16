# Query language

## Grammar

Loosest binding first:

```
query   := or
or      := and ( ("OR" | "||" | implicit) and )*
and     := unary ( ("AND" | "&&") unary )*
unary   := ("NOT" | "!" | "-" | "+")? primary ("^" number)?
primary := "(" query ")"
         | field ":" value
         | value
         | "*" [":" "*"]
value   := word
         | word "*"
         | word "~" [number]
         | quoted ["~" number]
         | range
range   := ("[" | "{") bound "TO" bound ("]" | "}")
bound   := number | "-" number | "*"
```
