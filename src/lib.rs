//! Minimal library surface for the Muse bootstrap crate.

/// Returns the crate name for a smoke-testable public API.
pub fn name() -> &'static str {
    "muse"
}

#[cfg(test)]
mod tests {
    use super::name;

    #[test]
    fn reports_its_name() {
        assert_eq!(name(), "muse");
    }
}
