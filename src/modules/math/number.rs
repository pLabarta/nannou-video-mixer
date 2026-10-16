use crate::ModuleChrome;

/// NUMBER: a constant float, with no inputs — drag its value box
/// left/right to change it.
pub struct NumberState {
    pub chrome: ModuleChrome,
    pub value: f32,
}

impl NumberState {
    pub fn new() -> Self {
        NumberState {
            chrome: ModuleChrome::new(),
            value: 0.0,
        }
    }
}
