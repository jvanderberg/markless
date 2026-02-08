//! Viewport management for scrolling.
//!
//! The [`Viewport`] struct tracks the visible area of the document
//! and handles all scroll operations.

use std::ops::Range;

/// Manages the visible portion of a document.
///
/// The viewport tracks:
/// - Terminal dimensions (width, height)
/// - Current scroll offset (in lines)
/// - Total document length
///
/// # Example
///
/// ```
/// use markless::ui::viewport::Viewport;
///
/// let mut vp = Viewport::new(80, 24, 100);
/// assert_eq!(vp.visible_range(), 0..24);
///
/// vp.scroll_down(10);
/// assert_eq!(vp.visible_range(), 10..34);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewport {
    width: u16,
    height: u16,
    offset: usize,
    total_lines: usize,
}

impl Viewport {
    /// Create a new viewport.
    ///
    /// # Arguments
    ///
    /// * `width` - Terminal width in columns
    /// * `height` - Terminal height in lines (for document area)
    /// * `total_lines` - Total lines in the document
    pub const fn new(width: u16, height: u16, total_lines: usize) -> Self {
        Self {
            width,
            height,
            offset: 0,
            total_lines,
        }
    }

    /// Get the current scroll offset.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Get the viewport width.
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Get the viewport height.
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Get the total number of lines in the document.
    pub const fn total_lines(&self) -> usize {
        self.total_lines
    }

    /// Get the range of visible lines.
    ///
    /// Returns a range from the current offset to offset + height,
    /// clamped to the document bounds.
    pub fn visible_range(&self) -> Range<usize> {
        let start = self.offset;
        let end = (self.offset + self.height as usize).min(self.total_lines);
        start..end
    }

    /// Get the scroll percentage (0-100).
    pub fn scroll_percent(&self) -> u8 {
        if self.total_lines == 0 {
            return 100;
        }

        let max_offset = self.max_offset();
        if max_offset == 0 {
            return 100;
        }

        // Percentage value always 0-100
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        {
            ((self.offset as f64 / max_offset as f64) * 100.0).round() as u8
        }
    }

    /// Check if we can scroll up.
    pub const fn can_scroll_up(&self) -> bool {
        self.offset > 0
    }

    /// Check if we can scroll down.
    pub const fn can_scroll_down(&self) -> bool {
        self.offset < self.max_offset()
    }

    /// Scroll up by n lines.
    pub const fn scroll_up(&mut self, n: usize) {
        self.offset = self.offset.saturating_sub(n);
    }

    /// Scroll down by n lines.
    pub fn scroll_down(&mut self, n: usize) {
        self.offset = (self.offset + n).min(self.max_offset());
    }

    /// Scroll up one page.
    pub const fn page_up(&mut self) {
        self.scroll_up(self.height as usize);
    }

    /// Scroll down one page.
    pub fn page_down(&mut self) {
        self.scroll_down(self.height as usize);
    }

    /// Scroll up half a page.
    pub const fn half_page_up(&mut self) {
        self.scroll_up(self.height as usize / 2);
    }

    /// Scroll down half a page.
    pub fn half_page_down(&mut self) {
        self.scroll_down(self.height as usize / 2);
    }

    /// Go to the beginning of the document.
    pub const fn go_to_top(&mut self) {
        self.offset = 0;
    }

    /// Go to the end of the document.
    pub const fn go_to_bottom(&mut self) {
        self.offset = self.max_offset();
    }

    /// Go to a specific line.
    ///
    /// The line will be positioned at the top of the viewport.
    pub fn go_to_line(&mut self, line: usize) {
        self.offset = line.min(self.max_offset());
    }

    /// Go to a percentage through the document.
    pub fn go_to_percent(&mut self, percent: u8) {
        let percent = percent.min(100);
        // Acceptable for scrollbar/progress calculation
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let target = (self.max_offset() as f64 * f64::from(percent) / 100.0).round() as usize;
        self.offset = target;
    }

    /// Resize the viewport.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        // Clamp offset if document is now shorter than viewport
        self.offset = self.offset.min(self.max_offset());
    }

    /// Update the total number of lines (e.g., after reload).
    pub fn set_total_lines(&mut self, total: usize) {
        self.total_lines = total;
        self.offset = self.offset.min(self.max_offset());
    }

    /// Calculate the maximum valid offset.
    const fn max_offset(&self) -> usize {
        self.total_lines.saturating_sub(self.height as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_viewport_starts_at_top() {
        let vp = Viewport::new(80, 24, 100);
        assert_eq!(vp.offset(), 0);
    }

    #[test]
    fn test_visible_range_at_top() {
        let vp = Viewport::new(80, 24, 100);
        assert_eq!(vp.visible_range(), 0..24);
    }

    #[test]
    fn test_visible_range_at_bottom() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.go_to_bottom();
        assert_eq!(vp.visible_range(), 76..100);
    }

    #[test]
    fn test_visible_range_with_short_document() {
        let vp = Viewport::new(80, 24, 10);
        assert_eq!(vp.visible_range(), 0..10);
    }

    #[test]
    fn test_scroll_down() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.scroll_down(10);
        assert_eq!(vp.offset(), 10);
    }

    #[test]
    fn test_scroll_down_clamps_to_max() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.scroll_down(1000);
        assert_eq!(vp.offset(), 76); // 100 - 24 = 76
    }

    #[test]
    fn test_scroll_up() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.scroll_down(50);
        vp.scroll_up(20);
        assert_eq!(vp.offset(), 30);
    }

    #[test]
    fn test_scroll_up_clamps_to_zero() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.scroll_down(10);
        vp.scroll_up(100);
        assert_eq!(vp.offset(), 0);
    }

    #[test]
    fn test_page_down() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.page_down();
        assert_eq!(vp.offset(), 24);
    }

    #[test]
    fn test_page_up() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.scroll_down(50);
        vp.page_up();
        assert_eq!(vp.offset(), 26);
    }

    #[test]
    fn test_half_page_down() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.half_page_down();
        assert_eq!(vp.offset(), 12);
    }

    #[test]
    fn test_half_page_up() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.scroll_down(50);
        vp.half_page_up();
        assert_eq!(vp.offset(), 38);
    }

    #[test]
    fn test_go_to_top() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.scroll_down(50);
        vp.go_to_top();
        assert_eq!(vp.offset(), 0);
    }

    #[test]
    fn test_go_to_bottom() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.go_to_bottom();
        assert_eq!(vp.offset(), 76);
    }

    #[test]
    fn test_go_to_line() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.go_to_line(50);
        assert_eq!(vp.offset(), 50);
    }

    #[test]
    fn test_go_to_line_clamps() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.go_to_line(1000);
        assert_eq!(vp.offset(), 76);
    }

    #[test]
    fn test_go_to_percent_zero() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.scroll_down(50);
        vp.go_to_percent(0);
        assert_eq!(vp.offset(), 0);
    }

    #[test]
    fn test_go_to_percent_fifty() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.go_to_percent(50);
        assert_eq!(vp.offset(), 38); // 50% of max_offset (76)
    }

    #[test]
    fn test_go_to_percent_hundred() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.go_to_percent(100);
        assert_eq!(vp.offset(), 76);
    }

    #[test]
    fn test_scroll_percent_at_top() {
        let vp = Viewport::new(80, 24, 100);
        assert_eq!(vp.scroll_percent(), 0);
    }

    #[test]
    fn test_scroll_percent_at_bottom() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.go_to_bottom();
        assert_eq!(vp.scroll_percent(), 100);
    }

    #[test]
    fn test_scroll_percent_empty_document() {
        let vp = Viewport::new(80, 24, 0);
        assert_eq!(vp.scroll_percent(), 100);
    }

    #[test]
    fn test_scroll_percent_short_document() {
        let vp = Viewport::new(80, 24, 10);
        assert_eq!(vp.scroll_percent(), 100);
    }

    #[test]
    fn test_can_scroll_up_at_top() {
        let vp = Viewport::new(80, 24, 100);
        assert!(!vp.can_scroll_up());
    }

    #[test]
    fn test_can_scroll_up_when_scrolled() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.scroll_down(10);
        assert!(vp.can_scroll_up());
    }

    #[test]
    fn test_can_scroll_down_at_top() {
        let vp = Viewport::new(80, 24, 100);
        assert!(vp.can_scroll_down());
    }

    #[test]
    fn test_can_scroll_down_at_bottom() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.go_to_bottom();
        assert!(!vp.can_scroll_down());
    }

    #[test]
    fn test_resize_keeps_valid_offset() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.scroll_down(50);
        vp.resize(80, 60);
        assert_eq!(vp.offset(), 40); // max_offset is now 40
    }

    #[test]
    fn test_set_total_lines_adjusts_offset() {
        let mut vp = Viewport::new(80, 24, 100);
        vp.scroll_down(80);
        vp.set_total_lines(50);
        assert_eq!(vp.offset(), 26); // max_offset is now 26
    }

    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn scroll_never_exceeds_bounds(
                total_lines in 1..10000usize,
                height in 1..100u16,
                scroll_amount in 0..10000usize,
            ) {
                let mut vp = Viewport::new(80, height, total_lines);
                vp.scroll_down(scroll_amount);

                let max = total_lines.saturating_sub(height as usize);
                prop_assert!(vp.offset() <= max);
            }

            #[test]
            fn scroll_up_never_negative(
                total_lines in 1..10000usize,
                height in 1..100u16,
                scroll_amount in 0..10000usize,
            ) {
                let mut vp = Viewport::new(80, height, total_lines);
                vp.scroll_up(scroll_amount);

                // offset is usize, can't be negative, but let's verify it's valid
                prop_assert!(vp.offset() <= total_lines);
            }

            #[test]
            fn visible_range_within_bounds(
                total_lines in 0..10000usize,
                height in 1..100u16,
                offset in 0..10000usize,
            ) {
                let mut vp = Viewport::new(80, height, total_lines);
                vp.scroll_down(offset);

                let range = vp.visible_range();
                prop_assert!(range.start <= range.end);
                prop_assert!(range.end <= total_lines);
            }

            #[test]
            fn percent_always_valid(
                total_lines in 0..10000usize,
                height in 1..100u16,
                offset in 0..10000usize,
            ) {
                let mut vp = Viewport::new(80, height, total_lines);
                vp.scroll_down(offset);

                let percent = vp.scroll_percent();
                prop_assert!(percent <= 100);
            }
        }
    }
}
