mod app;
mod bindings;
mod command_line;
mod dx_sample;
mod helpers;

pub use app::*;
pub use bindings::*;
pub use command_line::*;
pub use dx_sample::*;
pub use helpers::*;

use windows::core::Result;

fn main() -> Result<()> {
    // let factory = devices::create_factory()?;
    // adapter::print_adapter_info(&factory).unwrap();
    // let (_factory, device) = devices::create_device(&SampleCommandLine::default())?;
    // devices::check_sample_support(&device)?;
    // devices::test(&device);
    dx_sample::init_sample::<hello_triangle::Sample>()?;
    Ok(())
}
