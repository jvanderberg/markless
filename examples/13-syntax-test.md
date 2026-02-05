# Markdown Syntax Edge Cases

## Escaping Special Characters

\*Not italic\*

\**Not bold\**

\# Not a heading

\- Not a list item

\`Not code\`

\[Not a link\](url)

\> Not a blockquote

## Code Block Edge Cases

### Empty Code Block

```
```

### Code Block with Only Whitespace

```



```

### Code Block with Backticks Inside

```markdown
Here is some `inline code` in a code block.
And a fenced block inside:
~~~
nested?
~~~
```

### Very Long Lines in Code

```
This is a very long line of code that exceeds the typical terminal width and should test how the viewer handles horizontal overflow in code blocks without wrapping the content inappropriately since code formatting matters.
```

### Code with Special Characters

```html
<div class="container">
    <p>Hello &amp; welcome!</p>
    <script>if (x < 10 && y > 5) { alert("Test"); }</script>
</div>
```

## Link Edge Cases

### Links with Special Characters in URL

[Query string](https://example.com/search?q=hello+world&lang=en)

[Fragment](https://example.com/page#section-name)

[Encoded](https://example.com/path%20with%20spaces)

### Adjacent Links

[Link1](url1)[Link2](url2)[Link3](url3)

### Links Spanning Lines

[This is a very long link text
that spans multiple lines](https://example.com)

## List Edge Cases

### Single Item Lists

- Just one item

1. Only one

### Empty Items Between

- Item 1

- Item 2

- Item 3

### Starting at Different Numbers

5. Starting at five
6. Continues
7. Onward

100. Big number
101. Continues

## Table Edge Cases

### Single Column Table

| Header |
|--------|
| Data 1 |
| Data 2 |

### Single Row Table

| A | B | C | D | E |
|---|---|---|---|---|
| 1 | 2 | 3 | 4 | 5 |

### Table with Empty Cells

| A | B | C |
|---|---|---|
| 1 |   | 3 |
|   | 2 |   |
| 1 | 2 | 3 |

### Pipe Characters in Table

| Pattern | Example |
|---------|---------|
| OR | `a \| b` |
| Pipe | `\|` |

## Blockquote Edge Cases

> Single line quote

>
> Quote with empty first line

> > > Triple nested from start

## Inline Formatting Edge Cases

**Bold at start** and end **bold**

*Italic * with space before close

**Bold with *nested italic* inside**

`code with **bold** that doesn't render`

mid**word**bold and mid*word*italic

__under__score and **aster**isk

## Whitespace Handling

Text with  two  spaces  between  words.

Text with		tabs		between		words.

   Text with leading spaces.

Text with trailing spaces.

## HTML Entities (if supported)

&copy; &trade; &reg;
&lt; &gt; &amp;
&nbsp;(non-breaking space)
&mdash; (em dash)
&ndash; (en dash)

## Very Long Content

### Long Heading That Goes On And On And Should Test How The Viewer Handles Headings That Exceed Typical Width

Superlongwordwithoutanyspacesthatmightcauseissueswithwrappingoroverflowinsometerminalsorviewersthatneedtohandlethis.

### Long URL

Check out [this very long URL](https://example.com/path/to/some/very/deeply/nested/resource/that/has/many/path/segments/and/might/cause/wrapping/issues?with=query&parameters=included&and=more&stuff=here)
