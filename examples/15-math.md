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

```math
\mathbf{A} \mathbf{x} = \mathbf{b} \implies
\begin{pmatrix}
a_{11} & a_{12} & \cdots & a_{1n} \\
a_{21} & a_{22} & \cdots & a_{2n} \\
\vdots & \vdots & \ddots & \vdots \\
a_{m1} & a_{m2} & \cdots & a_{mn}
\end{pmatrix}
\begin{bmatrix}
x_1 \\ x_2 \\ \vdots \\ x_n
\end{bmatrix}
=
\begin{bmatrix}
b_1 \\ b_2 \\ \vdots \\ b_m
\end{bmatrix}
```

**The Cauchy-Schwarz Inequality**\
$$\left( \sum_{k=1}^n a_k b_k \right)^2 \leq \left( \sum_{k=1}^n a_k^2 \right) \left( \sum_{k=1}^n b_k^2 \right)$$

## Real-World Expressions (from Turso Test Statistics)

Instability score (status flips):

$$F_c = \sum_{i=2}^{n} \mathbf{1}[x_i \ne x_{i-1}]$$

Failure recurrence count:

$$N_f = \sum_{e \in E} \mathbf{1}[g(e) = f]$$

Cross-test spread:

$$A_f = \left| \{ c(e) : g(e) = f \} \right|$$

Mean runtime:

$$\mu_c = \frac{1}{n} \sum_{i=1}^{n} d_i$$

Sample variance:

$$s_c^2 = \frac{1}{n-1} \sum_{i=1}^{n} (d_i - \mu_c)^2$$

Coefficient of variation:

$$\mathrm{CV}_c = \frac{s_c}{\mu_c}$$

Daily pass-rate trend by source:

$$q_{t,s} = \frac{\left| \{ e \in E : d(e) = t \wedge u(e) = s \wedge \sigma(e) = \mathrm{PASSED} \} \right|}{\left| \{ e \in E : d(e) = t \wedge u(e) = s \} \right|}$$

Run-level pass rate:

$$q_r = \frac{\left| \{ e \in E : \rho(e) = r \wedge \sigma(e) = \mathrm{PASSED} \} \right|}{\left| \{ e \in E : \rho(e) = r \} \right|}$$

Failure cohort:

$$\mathcal{F} = \{ e \in E : \mathrm{status}(e) = \texttt{FAILED} \}$$

Failure association rate:

$$P(z \mid \mathcal{F}) \approx \frac{\left| \{ e \in \mathcal{F} : z \text{ is attached to } e \} \right|}{|\mathcal{F}|}$$