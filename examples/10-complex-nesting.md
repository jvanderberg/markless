# Complex Nesting and Edge Cases

## Deeply Nested Lists

1. First level
   1. Second level
      1. Third level
         1. Fourth level
            1. Fifth level
         2. Back to fourth
      2. Back to third
   2. Back to second
2. Back to first

## Mixed Nested Content

* List item with a block quote:

  > This quote is inside a list item.
  > It can span multiple lines.

* List item with code:

  ```rust
  fn nested_in_list() {
      println!("Code inside list");
  }
  ```

* List item with a table:

  | Col A | Col B |
  |-------|-------|
  | Data | Data |

## Block Quote with Everything

> # Heading in Quote
>
> Regular paragraph in quote.
>
> * List in quote
> * Another item
>   * Nested in quote
>
> ```python
> def code_in_quote():
>     pass
> ```
>
> | Table | In | Quote |
> |-------|-----|-------|
> | A | B | C |
>
> > Nested quote
> > > Double nested quote

## Adjacent Elements

**Bold text immediately followed by:**
*Italic text immediately followed by:*
`inline code immediately followed by:`
~~strikethrough~~

---

Text right after horizontal rule.

```
Code right after horizontal rule
```

---

> Quote right after horizontal rule

## Long Lines and Wrapping

This is an extremely long line that should test how the terminal handles text wrapping when the content exceeds the available width of the display and needs to flow onto multiple visual lines while still being considered a single paragraph in the markdown source.

## Special Characters

| Character | Name | Example |
|-----------|------|---------|
| & | Ampersand | Tom & Jerry |
| < | Less than | 5 < 10 |
| > | Greater than | 10 > 5 |
| " | Quote | She said "hello" |
| ' | Apostrophe | It's working |
| \| | Pipe | Column \| Data |
| \\ | Backslash | C:\\Users |
| \* | Asterisk | 5 \* 3 = 15 |
| \_ | Underscore | file\_name |
| \` | Backtick | \`code\` |

## Unicode Content

### Emojis

🚀 Rocket launch!
📝 Taking notes
✅ Task complete
❌ Task failed
⚠️ Warning
💡 Idea
🔧 Tool
📊 Chart

### International Text

**Chinese:** 你好世界
**Japanese:** こんにちは世界
**Korean:** 안녕하세요 세계
**Russian:** Привет мир
**Arabic:** مرحبا بالعالم
**Greek:** Γειά σου Κόσμε

### Math Symbols

∀x ∈ ℝ: x² ≥ 0
∑(i=1 to n) i = n(n+1)/2
∫₀^∞ e^(-x²) dx = √π/2
∂f/∂x = lim(h→0) [f(x+h) - f(x)]/h

## Empty Elements

### Empty List Items

*
* Item with content
*
* Another item

### Minimal Content

#

##

*a*

**b**

`c`
