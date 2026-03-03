# Math Rendering Examples

Markless supports LaTeX math via `$...$` for inline, `$$...$$` for display,
and ` ```math ` code fences.

## Inline Math

Einstein's famous equation $E = mc^2$ changed physics forever.

Greek letters work inline: $\alpha + \beta = \gamma$.

The quadratic formula gives $x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}$.

A subscript example: $a_1, a_2, \ldots, a_n$.

## Display Math ($$)

The Euler identity:

$$e^{i\pi} + 1 = 0$$

A summation:

$$\sum_{k=1}^{n} k = \frac{n(n+1)}{2}$$

An integral:

$$\int_0^\infty e^{-x^2} dx = \frac{\sqrt{\pi}}{2}$$

Matrix notation:

$$A = \begin{pmatrix} a & b \\ c & d \end{pmatrix}$$

## Math Code Fence

```math
\nabla \times \mathbf{E} = -\frac{\partial \mathbf{B}}{\partial t}
```

```math
f(x) = \sum_{n=0}^{\infty} \frac{f^{(n)}(0)}{n!} x^n
```

## Code Math (backtick syntax)

The code-math syntax $`x^2 + y^2 = r^2`$ also works.

## Mixed Content

Consider a function $f: \mathbb{R} \to \mathbb{R}$ defined by:

$$f(x) = \begin{cases} x^2 & \text{if } x \geq 0 \\ -x & \text{if } x < 0 \end{cases}$$

This is a **piecewise** function where $f(0) = 0$.
