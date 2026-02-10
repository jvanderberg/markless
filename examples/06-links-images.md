# Links and Images

## Inline Links

[Visit GitHub](https://github.com)

[Link with title](https://example.com "Example Website")

This sentence has [a link](https://example.com) in the middle.

## Reference Links

[GitHub][github-link]
[Rust Programming Language][rust]
[Example][]

[github-link]: https://github.com
[rust]: https://www.rust-lang.org "Rust Homepage"
[Example]: https://example.com

## Autolinks (GFM Extension)

Visit https://github.com for code hosting.

Send email to user@example.com for support.

Check out www.example.com for more info.

## URLs in Angle Brackets

<https://github.com>

<user@example.com>

## Links with Formatting

[**Bold link text**](https://example.com)

[*Italic link text*](https://example.com)

[`Code link text`](https://example.com)

[Link with ~~strikethrough~~](https://example.com)

## Images

![Sample Image](sample-image.png)

![Square Image](square.png "A square image")

## Reference Style Images

![Icon][icon-image]

[icon-image]: icon.png "Small Icon"

## Wide Banner Image

![Wide Banner](banner.png)

## Portrait Image

![Portrait](portrait.png)

## Inline Image in Paragraph

Here is some text with an inline image ![icon](icon.png) embedded in the middle of a sentence.

## Multiple Images

![Sample](sample-image.png) ![Square](square.png) ![Icon](icon.png)

## HTML Images

Basic HTML image tag:

<img src="sample-image.png" alt="Sample image via HTML">

Self-closing with explicit alt text:

<img src="icon.png" alt="App icon" />

Image inside a centered div (common in GitHub READMEs):

<div style="text-align: center;">
  <img src="banner.png" alt="Centered banner">
</div>

Image with height attribute (from issue #11):

<div style="text-align: center;">
  <img src="portrait.png" height=800>
</div>

Multiple HTML images in one block:

<p>
  <img src="square.png" alt="First">
  <img src="icon.png" alt="Second">
</p>
