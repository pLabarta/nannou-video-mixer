use crate::ModuleChrome;

/// LFO: oscillates between 0 and 1 at `freq` Hz, no inputs — drag the
/// frequency box left/right to change it.
pub struct LfoState {
    pub chrome: ModuleChrome,
    pub freq: f32,
}

impl LfoState {
    pub fn new() -> Self {
        LfoState {
            chrome: ModuleChrome::new(),
            freq: 1.0,
        }
    }
}
