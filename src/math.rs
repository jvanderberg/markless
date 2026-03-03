//! LaTeX math rendering.
//!
//! Converts LaTeX math expressions to either Unicode text approximations
//! (for inline math and image-less fallback) or raster images via the
//! Typst pipeline: LaTeX → mitex → Typst → SVG → resvg → raster.
//!
//! Fonts are bundled via `typst-assets` (New Computer Modern Math + others)
//! and cached in a `OnceLock` so they're loaded exactly once.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;

use anyhow::Result;
use image::DynamicImage;

/// Convert LaTeX math to a Unicode text approximation.
///
/// Uses the `unicodeit` crate to replace common LaTeX commands with
/// their Unicode equivalents (e.g. `\alpha` → `α`, `x^2` → `x²`),
/// then applies post-processing for commands `unicodeit` doesn't handle
/// (e.g. `\frac{a}{b}` → `(a)/(b)`, `\sqrt{x}` → `√(x)`).
pub fn latex_to_unicode(latex: &str) -> String {
    let preprocessed = preprocess_for_unicode(latex);
    let text = unicodeit::replace(&preprocessed);
    simplify_remaining_commands(&text)
}

/// Pre-process LaTeX before `unicodeit` to handle commands it doesn't know.
///
/// Strips sizing commands (`\left`, `\right`, `\big`, etc.) and unwraps
/// text-style commands (`\text{...}`, `\mathrm{...}`, `\mathbf{...}`, etc.)
/// so that `unicodeit` sees clean input without partial-match hazards
/// (e.g. `\left` being consumed as `\le` + `ft`).
fn preprocess_for_unicode(latex: &str) -> String {
    let mut result = latex.to_string();

    // Strip sizing commands: \left, \right, \big, \Big, \bigg, \Bigg
    // These are followed by a delimiter character and are purely visual.
    for cmd in &["\\Bigg", "\\bigg", "\\Big", "\\big", "\\left", "\\right"] {
        result = result.replace(cmd, "");
    }

    // Replace commands that unicodeit misses.
    // unicodeit handles \ldots and \cdots but not \dots.
    result = result.replace("\\dots", "…");

    // Unwrap text-style commands that unicodeit doesn't handle.
    // Note: \mathcal, \mathbb, \mathfrak, \mathbf, \mathit are handled by
    // unicodeit (e.g. \mathcal{F}→ℱ, \mathbb{R}→ℝ) so we leave those alone.
    for cmd in &[
        "\\text",
        "\\mathrm",
        "\\operatorname",
        "\\textbf",
        "\\textit",
        "\\textrm",
        "\\textsf",
        "\\texttt",
    ] {
        result = unwrap_braced_command(&result, cmd);
    }

    result
}

/// Replace all occurrences of `\cmd{content}` with just `content`.
fn unwrap_braced_command(text: &str, cmd: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(pos) = rest.find(cmd) {
        result.push_str(&rest[..pos]);
        let after_cmd = &rest[pos + cmd.len()..];

        // Must be followed by '{' (possibly after optional whitespace)
        let trimmed = after_cmd.trim_start();
        if trimmed.starts_with('{') {
            let brace_start = after_cmd.len() - trimmed.len() + 1; // skip '{'
            let inside = &after_cmd[brace_start..];
            if let Some((content, remainder)) = extract_braced_content(inside) {
                result.push_str(content);
                rest = remainder;
                continue;
            }
        }
        // No valid braced content — keep the command as-is
        result.push_str(cmd);
        rest = after_cmd;
    }
    result.push_str(rest);
    result
}

/// Post-process `unicodeit` output to simplify leftover LaTeX commands.
///
/// Handles `\frac{num}{den}` → `(num)/(den)` and `\sqrt{x}` → `√(x)`.
fn simplify_remaining_commands(text: &str) -> String {
    let mut result = text.to_string();

    // \frac{num}{den} → (num)/(den)
    while let Some(pos) = result.find("\\frac{") {
        if let Some(replaced) = replace_frac(&result, pos) {
            result = replaced;
        } else {
            break;
        }
    }

    // \sqrt{x} → √(x) — unicodeit converts \sqrt to √ but leaves the braces
    // The output looks like "√{x}" — convert to "√(x)"
    result = result.replace("√{", "√(");
    // Close the matching brace for each √( we just created
    // Simple approach: replace the next } after each √( with )
    let mut out = String::with_capacity(result.len());
    let mut depth = 0i32;
    let mut in_sqrt = false;
    for ch in result.chars() {
        if ch == '√' {
            in_sqrt = true;
            out.push(ch);
        } else if in_sqrt && ch == '(' {
            depth = 1;
            in_sqrt = false;
            out.push(ch);
        } else if depth > 0 {
            match ch {
                '{' | '(' => {
                    depth += 1;
                    out.push(ch);
                }
                '}' | ')' => {
                    depth -= 1;
                    out.push(')');
                }
                _ => out.push(ch),
            }
        } else {
            in_sqrt = false;
            out.push(ch);
        }
    }

    out
}

/// Replace `\frac{num}{den}` starting at `pos` with `(num)/(den)`.
/// Returns `None` if the braces don't balance.
fn replace_frac(text: &str, pos: usize) -> Option<String> {
    let after_frac = &text[pos + 6..]; // skip "\frac{"
    let (num, rest) = extract_braced_content(after_frac)?;
    // rest should start with "{"
    let rest = rest.strip_prefix('{')?;
    let (den, remainder) = extract_braced_content(rest)?;

    let mut result = String::with_capacity(text.len());
    result.push_str(&text[..pos]);
    result.push('(');
    result.push_str(num);
    result.push_str(")/(");
    result.push_str(den);
    result.push(')');
    result.push_str(remainder);
    Some(result)
}

/// Extract content inside balanced braces.
/// Input should be the text AFTER the opening `{`.
/// Returns `(content, rest_after_closing_brace)`.
fn extract_braced_content(text: &str) -> Option<(&str, &str)> {
    let mut depth = 1u32;
    for (i, ch) in text.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&text[..i], &text[i + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

/// Render LaTeX math to an SVG string.
///
/// Pipeline: LaTeX → mitex (Typst math syntax) → Typst → typst-svg → SVG.
///
/// # Errors
///
/// Returns an error if the LaTeX cannot be converted or compiled.
pub fn render_to_svg(latex: &str) -> Result<String> {
    catch_unwind(AssertUnwindSafe(|| render_to_svg_inner(latex)))
        .unwrap_or_else(|_| Err(anyhow::anyhow!("math rendering panicked")))
}

/// Render LaTeX math to a raster image.
///
/// Rasterizes at a fixed scale relative to the equation's natural size
/// (from the Typst SVG output) rather than stretching to fill
/// `max_width_px`.  This keeps small equations small and large ones
/// proportionally larger.  The result is capped at `max_width_px` so
/// very wide equations still fit the viewport.
///
/// # Errors
///
/// Returns an error if the LaTeX cannot be rendered or the SVG
/// cannot be rasterized.
pub fn render_to_image(latex: &str, max_width_px: u32) -> Result<DynamicImage> {
    let svg = render_to_svg(latex)?;

    // Parse the SVG natural width (in pt) to compute an appropriate
    // rasterization width.  We use a fixed scale factor so that 16pt
    // math text renders at a legible pixel size without being blown up
    // to fill the whole viewport.
    let natural_width_px = svg_natural_width_px(&svg);
    let target = natural_width_px.unwrap_or(max_width_px).min(max_width_px);

    crate::svg::rasterize_svg(&svg, target)
}

/// Pixels per SVG pt – chosen so 16 pt math text ≈ 48 px, which is
/// legible on typical terminal font sizes (14–18 px cell height) at
/// about 3 terminal rows per math line.
const PX_PER_PT: f64 = 3.0;

/// Parse the SVG `width` attribute (in pt) and return the natural
/// width in pixels at [`PX_PER_PT`] scale.
fn svg_natural_width_px(svg: &str) -> Option<u32> {
    // Typst emits: width="123.45pt"
    let marker = "width=\"";
    let start = svg.find(marker)? + marker.len();
    let rest = &svg[start..];
    let end = rest.find("pt\"")?;
    let natural_pt: f64 = rest[..end].parse().ok()?;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let natural_pixels = (natural_pt * PX_PER_PT).ceil() as u32;
    Some(natural_pixels.max(1))
}

/// Static portion of the Typst preamble — helper functions that `mitex`
/// emits but Typst doesn't define natively.
///
/// These correspond to entries in the mitex `standard.typ` spec that use
/// `alias:` to generate custom function names.  The coverage is verified
/// by `test_preamble_covers_all_mitex_aliases`.
const TYPST_PREAMBLE: &str = "\
// -- Text / font styling --\n\
#let textmath(it) = it\n\
#let textbf(it) = math.bold(it)\n\
#let textit(it) = math.italic(it)\n\
#let textmd(it) = it\n\
#let textnormal(it) = it\n\
#let textrm(it) = math.upright(it)\n\
#let textsf(it) = math.sans(it)\n\
#let texttt(it) = math.mono(it)\n\
#let textup(it) = math.upright(it)\n\
#let mitexmathbf(it) = math.bold(math.upright(it))\n\
#let mitexbold(it) = math.bold(math.upright(it))\n\
#let mitexupright(it) = math.upright(it)\n\
#let mitexitalic(it) = math.italic(it)\n\
#let mitexsans(it) = math.sans(it)\n\
#let mitexfrak(it) = math.frak(it)\n\
#let mitexmono(it) = math.mono(it)\n\
#let mitexcal(it) = math.cal(it)\n\
// -- Display style --\n\
#let mitexdisplay(it) = math.display(it)\n\
#let mitexinline(it) = math.inline(it)\n\
#let mitexscript(it) = math.script(it)\n\
#let mitexsscript(it) = math.sscript(it)\n\
// -- sqrt with optional root index --\n\
#let mitexsqrt(..args) = {\n\
  let a = args.pos()\n\
  if a.len() == 2 { math.root(a.at(0), a.at(1)) }\n\
  else if a.len() > 0 { math.sqrt(a.at(0)) }\n\
}\n\
// -- Matrix environments --\n\
#let pmatrix(..args) = math.mat(delim: \"(\", ..args)\n\
#let bmatrix(..args) = math.mat(delim: \"[\", ..args)\n\
#let Bmatrix(..args) = math.mat(delim: \"{\", ..args)\n\
#let vmatrix(..args) = math.mat(delim: \"|\", ..args)\n\
#let Vmatrix(..args) = math.mat(delim: \"||\", ..args)\n\
#let mitexarray(..args) = math.mat(..args)\n\
// -- Alignment environments --\n\
#let aligned(..args) = math.display(math.mat(delim: none, ..args))\n\
#let alignedat(..args) = math.display(math.mat(delim: none, ..args))\n\
#let rcases(..args) = math.cases(reverse: true, ..args)\n\
// -- Delimiter sizing --\n\
#let big(it) = math.lr(size: 1.2em, it)\n\
#let bigg(it) = math.lr(size: 2.4em, it)\n\
#let Big(it) = math.lr(size: 1.8em, it)\n\
#let Bigg(it) = math.lr(size: 3em, it)\n\
// -- Over/under braces and brackets --\n\
#let mitexoverbrace(..args) = math.overbrace(..args)\n\
#let mitexunderbrace(..args) = math.underbrace(..args)\n\
#let mitexoverbracket(..args) = math.overbracket(..args)\n\
#let mitexunderbracket(..args) = math.underbracket(..args)\n\
// -- Color --\n\
#let mitexcolor(it) = it\n\
#let colortext(..args) = {\n\
  let a = args.pos()\n\
  if a.len() >= 2 { a.at(1) } else if a.len() >= 1 { a.at(0) }\n\
}\n\
// -- Operators --\n\
#let operatornamewithlimits(it) = math.op(it, limits: true)\n\
#let atop(num, den) = math.frac(num, den)\n\
// -- Document commands (no-ops in math-only context) --\n\
#let mitexcite(it) = it\n\
#let mitexref(it) = it\n\
#let mitexlabel(it) = none\n\
#let mitexcaption(it) = it\n\
#let miteximage(..args) = none\n\
#let bottomrule = none\n\
#let midrule = none\n\
#let toprule = none\n\
// -- Bra-ket notation --\n\
#let brace(it) = math.lr(size: auto, [{] + it + [}])\n\
#let brack(it) = math.lr(size: auto, $[$ + it + $]$)\n";

/// Build the page/text setup directives for the Typst source.
///
/// The background is always transparent (`fill: none`) so the terminal
/// background shows through.  Text color is white on dark terminals and
/// black on light ones.
fn typst_page_setup() -> String {
    let text_color = if crate::highlight::is_light_background() {
        "black"
    } else {
        "white"
    };
    format!(
        "#set page(width: auto, height: auto, margin: 8pt, fill: none)\n\
         #set text(size: 16pt, fill: {text_color})\n"
    )
}

/// Inner SVG rendering (called inside `catch_unwind`).
fn render_to_svg_inner(latex: &str) -> Result<String> {
    // Convert LaTeX to Typst math syntax via mitex
    let typst_math = mitex::convert_math(latex, None)
        .map_err(|e| anyhow::anyhow!("mitex conversion failed: {e}"))?;

    // Wrap in a minimal Typst document that renders just the math.
    // Page fill is transparent, text color matches the terminal theme.
    let page_setup = typst_page_setup();
    let typst_source = format!("{TYPST_PREAMBLE}{page_setup}$ {typst_math} $");

    compile_typst_to_svg(&typst_source)
}

/// Compile a Typst source document to SVG.
fn compile_typst_to_svg(source: &str) -> Result<String> {
    use typst::layout::PagedDocument;

    let world = MathWorld::new(source);

    let warned = typst::compile::<PagedDocument>(&world);
    let document = warned.output.map_err(|diagnostics| {
        let msgs: Vec<String> = diagnostics.iter().map(|d| d.message.to_string()).collect();
        anyhow::anyhow!("Typst compilation failed: {}", msgs.join("; "))
    })?;

    let page = document
        .pages
        .first()
        .ok_or_else(|| anyhow::anyhow!("Typst produced no pages"))?;
    let svg = typst_svg::svg(page);
    Ok(svg)
}

/// Bundled fonts cached for the lifetime of the process.
struct CachedFonts {
    book: typst::utils::LazyHash<typst::text::FontBook>,
    fonts: Vec<typst::text::Font>,
}

/// Return the bundled font set, loading it once on first call.
fn cached_fonts() -> &'static CachedFonts {
    static FONTS: OnceLock<CachedFonts> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut book = typst::text::FontBook::new();
        let mut fonts = Vec::new();

        for data in typst_assets::fonts() {
            let buffer = typst::foundations::Bytes::new(data);
            for font in typst::text::Font::iter(buffer) {
                book.push(font.info().clone());
                fonts.push(font);
            }
        }

        CachedFonts {
            book: typst::utils::LazyHash::new(book),
            fonts,
        }
    })
}

/// Minimal Typst world implementation for math rendering.
///
/// Fonts come from the bundled `typst-assets` set (New Computer Modern Math
/// and others), cached in a process-wide `OnceLock`. Only the source text
/// changes per compilation.
struct MathWorld {
    source: typst::syntax::Source,
    library: typst::utils::LazyHash<typst::Library>,
}

impl MathWorld {
    fn new(source_text: &str) -> Self {
        use typst::LibraryExt;

        Self {
            source: typst::syntax::Source::detached(source_text),
            library: typst::utils::LazyHash::new(typst::Library::default()),
        }
    }
}

impl typst::World for MathWorld {
    fn library(&self) -> &typst::utils::LazyHash<typst::Library> {
        &self.library
    }

    fn book(&self) -> &typst::utils::LazyHash<typst::text::FontBook> {
        &cached_fonts().book
    }

    fn main(&self) -> typst::syntax::FileId {
        self.source.id()
    }

    fn source(&self, _id: typst::syntax::FileId) -> typst::diag::FileResult<typst::syntax::Source> {
        Ok(self.source.clone())
    }

    fn file(
        &self,
        _id: typst::syntax::FileId,
    ) -> typst::diag::FileResult<typst::foundations::Bytes> {
        Err(typst::diag::FileError::AccessDenied)
    }

    fn font(&self, index: usize) -> Option<typst::text::Font> {
        cached_fonts().fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<i64>) -> Option<typst::foundations::Datetime> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Extracts every custom function alias from the mitex DEFAULT_SPEC
    /// (filtering out Typst builtins and symbol paths) and verifies our
    /// preamble defines it.
    #[test]
    fn test_preamble_covers_all_mitex_aliases() {
        use mitex_spec::{ArgPattern, ArgShape, CommandSpecItem};

        let spec = &*mitex_spec_gen::DEFAULT_SPEC;

        // Collect aliases that are custom functions (not Typst symbol paths).
        // Symbol aliases like "arrow.r" or "gt.eq" are valid Typst symbols
        // in math mode and don't need #let definitions.
        let mut custom_aliases: Vec<String> = Vec::new();
        for (_name, item) in spec.items() {
            match item {
                CommandSpecItem::Cmd(cmd) => {
                    let Some(alias) = cmd.alias.as_deref() else {
                        continue;
                    };
                    // Commands with arguments are function calls, need definitions.
                    // Commands with no arguments are symbol references (builtins).
                    let has_args = !matches!(
                        cmd.args,
                        ArgShape::Right {
                            pattern: ArgPattern::None
                        }
                    );
                    if has_args {
                        let clean = alias.strip_prefix('#').unwrap_or(alias);
                        custom_aliases.push(clean.to_string());
                    }
                }
                CommandSpecItem::Env(env) => {
                    if let Some(alias) = env.alias.as_deref() {
                        let clean = alias.strip_prefix('#').unwrap_or(alias);
                        custom_aliases.push(clean.to_string());
                    }
                }
            }
        }
        custom_aliases.sort();
        custom_aliases.dedup();

        // Typst builtins that don't need preamble definitions —
        // these are real Typst functions/accents available in math mode.
        let builtins: std::collections::HashSet<&str> = [
            // Math functions
            "accent",
            "attach",
            "bb",
            "binom",
            "bold",
            "box",
            "cal",
            "cancel",
            "cases",
            "circle",
            "class",
            "display",
            "emph",
            "equation",
            "figure",
            "footnote",
            "frac",
            "frak",
            "grid",
            "heading(level: 1)",
            "heading(level: 2)",
            "heading(level: 3)",
            "hide",
            "inline",
            "italic",
            "limits",
            "lr",
            "mat",
            "math.equation",
            "mono",
            "op",
            "overbrace",
            "overline",
            "primes",
            "quote(block: true)",
            "root",
            "sans",
            "script",
            "scripts",
            "sqrt",
            "sscript",
            "strong",
            "table",
            "text",
            "underbrace",
            "underline",
            "upright",
            "vec",
            // Accent functions (valid in Typst math mode)
            "acute",
            "acute.double",
            "arrow",
            "arrow.l",
            "breve",
            "caron",
            "dot",
            "dot.double",
            "dot.triple",
            "dot.quad",
            "grave",
            "hat",
            "macron",
            "tilde",
        ]
        .into_iter()
        .collect();

        let mut missing: Vec<&str> = Vec::new();
        for alias in &custom_aliases {
            if builtins.contains(alias.as_str()) {
                continue;
            }
            // Check it's defined in our preamble
            let def_pattern = format!("#let {alias}");
            if !TYPST_PREAMBLE.contains(&def_pattern) {
                missing.push(alias);
            }
        }

        if !missing.is_empty() {
            panic!(
                "TYPST_PREAMBLE is missing definitions for these mitex aliases: {missing:?}\n\
                 Add #let definitions for each to avoid 'unknown variable' errors at runtime."
            );
        }
    }

    #[test]
    fn test_latex_to_unicode_left_right_delimiters() {
        // \left and \right are sizing commands — should be stripped,
        // leaving the delimiter character intact.
        let result = latex_to_unicode("\\left| x \\right|");
        assert_eq!(result, "| x |");
    }

    #[test]
    fn test_latex_to_unicode_text_command() {
        // \text{...} should extract its content
        let result = latex_to_unicode("x \\text{ if } y");
        assert_eq!(result, "x  if  y");
    }

    #[test]
    fn test_latex_to_unicode_mathrm_command() {
        // \mathrm{...} should extract its content
        let result = latex_to_unicode("\\mathrm{CV}_c");
        assert_eq!(result, "CV_c");
    }

    #[test]
    fn test_latex_to_unicode_mathbf_command() {
        // unicodeit converts \mathbf to bold Unicode letters
        let result = latex_to_unicode("\\mathbf{E}");
        // Either bold 𝐄 or plain E — both are acceptable
        assert!(
            result == "𝐄" || result == "E",
            "\\mathbf{{E}} should produce bold or plain E, got: {result:?}"
        );
    }

    #[test]
    fn test_latex_to_unicode_big_delimiters() {
        // \big, \Big, \bigg, \Bigg are sizing — strip them
        let result = latex_to_unicode("\\big( x \\big)");
        assert_eq!(result, "( x )");
    }

    #[test]
    fn test_latex_to_unicode_turso_failure_rate() {
        // Real expression from Turso doc — should not produce "≤ft|"
        let result = latex_to_unicode(
            "P(z \\mid \\mathcal{F}) \\approx \\frac{\\left| \\{ e \\in \\mathcal{F} : z \\text{ is attached to } e \\} \\right|}{|\\mathcal{F}|}",
        );
        assert!(
            !result.contains("≤ft"),
            "\\left should not be partially matched as \\le"
        );
        assert!(!result.contains("\\text"), "\\text should be simplified");
        assert!(!result.contains("\\right"), "\\right should be stripped");
    }

    #[test]
    fn test_render_to_svg_transparent_background() {
        // The SVG should have no background rectangle — page fill is none.
        let svg = render_to_svg("x^2").unwrap();
        // Typst's default page fill is white, which produces a <rect> with
        // fill="#ffffff".  With fill: none, there should be no such rect.
        assert!(
            !svg.contains("<rect"),
            "SVG should not have a background <rect> (page fill should be none)"
        );
    }

    #[test]
    fn test_render_to_svg_text_color_matches_theme() {
        // In test context (no terminal), background_mode defaults to dark,
        // so text should be white (#ffffff).
        let svg = render_to_svg("x^2").unwrap();
        assert!(
            svg.contains("fill=\"#ffffff\""),
            "text glyphs should be white on dark background"
        );
    }

    #[test]
    fn test_render_to_image_small_equation_not_full_width() {
        // A small equation like E=mc² should NOT produce a 1200px-wide image.
        let img = render_to_image("E = mc^2", 1200).unwrap();
        assert!(
            img.width() < 600,
            "small equation should be much narrower than target_width, got {}px",
            img.width()
        );
    }

    #[test]
    fn test_render_to_image_matrix_wider_than_simple() {
        let simple = render_to_image("x^2", 1200).unwrap();
        let matrix =
            render_to_image("A = \\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}", 1200).unwrap();
        assert!(
            matrix.width() > simple.width(),
            "matrix ({}px) should be wider than simple expr ({}px)",
            matrix.width(),
            simple.width()
        );
    }

    #[test]
    fn test_latex_to_unicode_dots() {
        let result = latex_to_unicode("e_1, \\dots, e_n");
        assert!(
            !result.contains("\\dots"),
            "\\dots should be converted to ellipsis, got: {result:?}"
        );
        assert!(result.contains('…'), "should contain ellipsis character");
    }

    #[test]
    fn test_latex_to_unicode_greek() {
        let result = latex_to_unicode("\\alpha");
        assert_eq!(result, "α");
    }

    #[test]
    fn test_latex_to_unicode_superscript() {
        let result = latex_to_unicode("x^2");
        // unicodeit converts ^2 to superscript
        assert!(result.contains('²') || result.contains("x^2"));
    }

    #[test]
    fn test_latex_to_unicode_passthrough() {
        // Unknown commands should pass through
        let result = latex_to_unicode("hello world");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_latex_to_unicode_frac() {
        let result = latex_to_unicode("\\frac{a}{b}");
        assert_eq!(result, "(a)/(b)");
    }

    #[test]
    fn test_latex_to_unicode_quadratic() {
        let result = latex_to_unicode("\\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}");
        assert_eq!(result, "(−b ± √(b² − 4ac))/(2a)");
    }

    #[test]
    fn test_latex_to_unicode_sqrt() {
        let result = latex_to_unicode("\\sqrt{x}");
        assert_eq!(result, "√(x)");
    }

    #[test]
    fn test_latex_to_unicode_nested_frac() {
        let result = latex_to_unicode("\\frac{\\frac{a}{b}}{c}");
        assert_eq!(result, "((a)/(b))/(c)");
    }

    #[test]
    fn test_latex_to_unicode_ldots_subscripts() {
        let result = latex_to_unicode("a_1, a_2, \\ldots, a_n");
        assert_eq!(result, "a₁, a₂, …, aₙ");
    }

    #[test]
    fn test_render_to_svg_simple() {
        let result = render_to_svg("E = mc^2");
        assert!(
            result.is_ok(),
            "render_to_svg should succeed for simple math: {:?}",
            result.err()
        );
        let svg = result.unwrap();
        assert!(svg.contains("<svg"), "output should be valid SVG");
    }

    #[test]
    fn test_render_to_image_simple() {
        let result = render_to_image("x^2 + y^2", 400);
        assert!(
            result.is_ok(),
            "render_to_image should succeed: {:?}",
            result.err()
        );
        let img = result.unwrap();
        assert!(img.width() > 0);
        assert!(img.height() > 0);
    }

    #[test]
    fn test_display_math_after_line_break_in_list() {
        // From 16-Turso Test Statistics.md: display math inside a list item
        // after a hard line break.
        //
        // First check: does comrak parse $$...$$ as display_math in a list?
        let md = "- text:\\\n  $$q_r = \\frac{a}{b}$$\n";
        let doc = crate::document::Document::parse_with_mermaid_images(md, 80).unwrap();
        assert!(
            !doc.math_sources().is_empty(),
            "display math after line break should be detected as math source"
        );
    }

    #[test]
    fn test_display_math_after_blank_line_in_list() {
        // Display math in a list with a blank line separator
        let md = "- text:\n\n  $$q_r = \\frac{a}{b}$$\n";
        let doc = crate::document::Document::parse_with_mermaid_images(md, 80).unwrap();
        assert!(
            !doc.math_sources().is_empty(),
            "display math after blank line should be detected as math source"
        );
    }

    #[test]
    fn test_render_to_svg_turso_pass_rate() {
        // Real expression from 16-Turso Test Statistics.md
        let result = render_to_svg(
            "q_r = \\frac{\\left| \\{ e \\in E : \\rho(e) = r \\wedge \\sigma(e) = \\mathrm{PASSED} \\} \\right|}{\\left| \\{ e \\in E : \\rho(e) = r \\} \\right|}",
        );
        assert!(
            result.is_ok(),
            "Turso pass rate should render: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_render_to_svg_invalid_does_not_panic() {
        // The important thing is no panic, even on garbage input
        let _result = render_to_svg("}}}{{{\\invalid\\command");
    }

    #[test]
    fn test_render_to_svg_integral() {
        // Integral expressions need a MATH font table to compile
        let result = render_to_svg("\\int_0^\\infty e^{-x} dx");
        assert!(
            result.is_ok(),
            "integral should render to SVG: {:?}",
            result.err()
        );
        let svg = result.unwrap();
        assert!(svg.contains("<svg"), "output should be valid SVG");
    }

    #[test]
    fn test_render_to_svg_summation() {
        let result = render_to_svg("\\sum_{n=1}^{N} n^2");
        assert!(
            result.is_ok(),
            "summation should render to SVG: {:?}",
            result.err()
        );
        let svg = result.unwrap();
        assert!(svg.contains("<svg"), "output should be valid SVG");
    }

    #[test]
    fn test_render_to_image_integral() {
        let result = render_to_image("\\int_0^\\infty e^{-x} dx", 400);
        assert!(
            result.is_ok(),
            "integral should render to image: {:?}",
            result.err()
        );
        let img = result.unwrap();
        assert!(img.width() > 0);
        assert!(img.height() > 0);
    }

    #[test]
    fn test_render_to_svg_euler_identity_has_paths() {
        // With proper math fonts, variables render as <path> elements,
        // not "?" replacement glyphs
        let result = render_to_svg("e^{i\\pi} + 1 = 0");
        assert!(
            result.is_ok(),
            "Euler identity should compile: {:?}",
            result.err()
        );
        let svg = result.unwrap();
        // A properly rendered SVG has <path> elements for the glyphs
        assert!(
            svg.contains("<path"),
            "SVG should contain <path> elements for rendered math glyphs"
        );
    }

    #[test]
    fn test_render_to_svg_nabla_maxwell() {
        let result = render_to_svg(
            "\\nabla \\times \\mathbf{E} = -\\frac{\\partial \\mathbf{B}}{\\partial t}",
        );
        assert!(
            result.is_ok(),
            "Maxwell equation should render: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_render_to_svg_piecewise_cases() {
        let result = render_to_svg(
            "f(x) = \\begin{cases} x^2 & \\text{if } x \\geq 0 \\\\ -x & \\text{if } x < 0 \\end{cases}",
        );
        assert!(
            result.is_ok(),
            "piecewise function should render: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_render_to_image_gaussian_integral() {
        // Exact expression from examples/15-math.md display math
        let result = render_to_image("\\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}", 400);
        assert!(
            result.is_ok(),
            "gaussian integral should render to image: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_render_to_image_matrix() {
        let result = render_to_image("A = \\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}", 400);
        assert!(
            result.is_ok(),
            "matrix should render to image: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_render_all_example_display_math() {
        // All display math from examples/15-math.md
        let expressions = [
            "e^{i\\pi} + 1 = 0",
            "\\sum_{k=1}^{n} k = \\frac{n(n+1)}{2}",
            "\\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}",
            "A = \\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}",
        ];
        for expr in &expressions {
            let result = render_to_image(expr, 400);
            assert!(
                result.is_ok(),
                "expression {expr:?} should render: {:?}",
                result.err()
            );
        }
    }
}
