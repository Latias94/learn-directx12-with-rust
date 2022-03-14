#[derive(Clone)]
pub struct SampleCommandLine {
    /// WARP 意为 Windows Advanced Rasterization Platform（Windows 高级光栅化平台）。
    pub use_warp_device: bool,
}

impl Default for SampleCommandLine {
    fn default() -> Self {
        let mut use_warp_device = false;

        for arg in std::env::args() {
            if arg.eq_ignore_ascii_case("-warp") || arg.eq_ignore_ascii_case("/warp") {
                use_warp_device = true;
            }
        }

        SampleCommandLine { use_warp_device }
    }
}
