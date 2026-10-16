use crate::ModuleChrome;

/// MULT: multiplies its two number inputs. No config of its own — its box
/// just shows the live result.
pub struct MultState {
    pub chrome: ModuleChrome,
}

impl MultState {
    pub fn new() -> Self {
        MultState {
            chrome: ModuleChrome::new(),
        }
    }
}
