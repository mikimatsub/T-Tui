//! Settings screen state. The row layout and apply logic live in the app
//! module and the renderer; this only holds the current selection.

#[derive(Debug, Default)]
pub struct SettingsView {
    pub sel: usize,
}

/// Number of actionable rows in the settings list.
pub const ROW_COUNT: usize = 11;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_selection() {
        let v = SettingsView::default();
        assert_eq!(v.sel, 0);
        assert_eq!(ROW_COUNT, 11);
    }
}
