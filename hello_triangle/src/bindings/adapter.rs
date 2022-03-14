use crate::{wstrlens, MemoryDbgHelper};
use windows::Win32::Foundation;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::{core::*, Win32::Graphics::Dxgi::*};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct AdapterDesc {
    pub description: [u16; 128],
    pub vendor_id: u32,
    pub device_id: u32,
    pub subsys_id: u32,
    pub revision: u32,
    pub dedicated_video_memory: usize,
    pub dedicated_system_memory: usize,
    pub shared_system_memory: usize,
    pub adapter_luid: Foundation::LUID,
}

impl AdapterDesc {
    pub fn description(&self) -> String {
        let len = wstrlens(&self.description);
        String::from_utf16_lossy(&self.description[..len])
    }
}

impl From<DXGI_ADAPTER_DESC> for AdapterDesc {
    fn from(desc: DXGI_ADAPTER_DESC) -> AdapterDesc {
        unsafe { std::mem::transmute(desc) }
    }
}

impl From<AdapterDesc> for DXGI_ADAPTER_DESC {
    fn from(desc: AdapterDesc) -> DXGI_ADAPTER_DESC {
        unsafe { std::mem::transmute(desc) }
    }
}

impl std::fmt::Debug for AdapterDesc {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("AdapterDesc")
            .field("description", &self.description())
            .field("vendor_id", &self.vendor_id)
            .field("device_id", &self.device_id)
            .field("subsys_id", &self.subsys_id)
            .field("revision", &self.revision)
            .field(
                "dedicated_video_memory",
                &MemoryDbgHelper(self.dedicated_video_memory as u64),
            )
            .field(
                "dedicated_system_memory",
                &MemoryDbgHelper(self.dedicated_system_memory as u64),
            )
            .field(
                "shared_system_memory",
                &MemoryDbgHelper(self.shared_system_memory as u64),
            )
            .field("adapter_luid", &self.adapter_luid)
            .finish()
    }
}

/// 打印显卡调试信息
pub fn print_adapter_info(factory: &IDXGIFactory4) -> Result<()> {
    for i in 0.. {
        let adapter_result: Result<IDXGIAdapter1> = unsafe { factory.EnumAdapters1(i) };
        if let Ok(adapter) = adapter_result {
            let desc: DXGI_ADAPTER_DESC = unsafe { adapter.GetDesc()? };
            let adapter_desc: AdapterDesc = desc.into();
            println!("adapter: {:?}", adapter_desc);
        } else {
            break;
        }
    }
    Ok(())
}
/// 拿到硬件适配器
pub fn get_hardware_adapter(factory: &IDXGIFactory4) -> Result<IDXGIAdapter1> {
    for i in 0.. {
        let adapter = unsafe { factory.EnumAdapters1(i)? };

        let desc = unsafe { adapter.GetDesc1()? };

        if (DXGI_ADAPTER_FLAG(desc.Flags) & DXGI_ADAPTER_FLAG_SOFTWARE) != DXGI_ADAPTER_FLAG_NONE {
            // Don't select the Basic Render Driver adapter. If you want a
            // software adapter, pass in "/warp" on the command line.
            continue;
        }

        // Check to see whether the adapter supports Direct3D 12, but don't create the actual device yet.
        if unsafe {
            D3D12CreateDevice(
                &adapter,
                D3D_FEATURE_LEVEL_11_0,
                std::ptr::null_mut::<Option<ID3D12Device>>(),
            )
        }
        .is_ok()
        {
            return Ok(adapter);
        }
    }
    unreachable!()
}
