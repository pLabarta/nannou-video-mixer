use crate::ModuleChrome;

/// The OUTPUT module has no config of its own — just chrome and one input.
pub struct OutputState {
    pub chrome: ModuleChrome,
}

impl OutputState {
    pub fn new() -> Self {
        OutputState {
            chrome: ModuleChrome::new(),
        }
    }
}
