use crate::{adapter, SampleCommandLine};

use windows::{
    core::*, Win32::Graphics::Direct3D::Fxc::*, Win32::Graphics::Direct3D::*,
    Win32::Graphics::Direct3D12::*, Win32::Graphics::Dxgi::Common::*, Win32::Graphics::Dxgi::*,
};

/// 要初始化 Direct3D，必须先创建 Direct3D 12 设备（ID3D12Device）。
/// 此设备代表着一个显示适配器。一般来说，显示适配器是一种 3D 图形硬件（如显卡）。
/// Direct3D 12 设备既可检测系统环境对功能的支持情况，又能创建所有其他的 Direct3D 接口对象（如资源、视图和命令列表）。
pub fn create_device(command_line: &SampleCommandLine) -> Result<(IDXGIFactory4, ID3D12Device)> {
    // debug 开启调试
    if cfg!(debug_assertions) {
        unsafe {
            let mut debug: Option<ID3D12Debug> = None;
            if let Some(debug) = D3D12GetDebugInterface(&mut debug).ok().and(debug) {
                debug.EnableDebugLayer();
            }
        }
    }
    let dxgi_factory = create_factory()?;

    // 通过命令行来控制使用硬件适配器（如显卡），还是软件适配器。
    let adapter = if command_line.use_warp_device {
        unsafe { dxgi_factory.EnumWarpAdapter() }
    } else {
        adapter::get_hardware_adapter(&dxgi_factory)
    }?;

    let mut device: Option<ID3D12Device> = None;

    // 指定在创建设备时所用的显示适配器。若将此参数设定为空指针，则使用主显示适配器。
    // 我们在本书的示例中总是采用主适配器。在 4.1.10 节中，我们已展示了怎样枚举系统中所有的显示适配器。
    unsafe { D3D12CreateDevice(adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }?;
    // 调用 D3D12CreateDevice 失败后，程序将回退到一种软件适配器：WARP 设备。
    // if !command_line.use_warp_device && device.is_none() {
    //     adapter = unsafe { dxgi_factory.EnumWarpAdapter() }?;
    //     unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }?;
    // }

    Ok((dxgi_factory, device.unwrap()))
}

pub fn create_factory() -> Result<IDXGIFactory4> {
    let dxgi_factory_flags = if cfg!(debug_assertions) {
        DXGI_CREATE_FACTORY_DEBUG
    } else {
        0
    };

    // IDXGIFactory4 才开始包括 EnumWarpAdapter 函数。
    // CreateDXGIFactory2 包含了传递标志的功能，我们正在使用它来创建 DXGIFactory 的调试版本。
    unsafe { CreateDXGIFactory2(dxgi_factory_flags) }
}

pub fn check_sample_support(device: &ID3D12Device) -> Result<u32> {
    let mut features_architecture = D3D12_FEATURE_DATA_MULTISAMPLE_QUALITY_LEVELS {
        SampleCount: 4,
        Format: DXGI_FORMAT_R32G32B32A32_UINT,
        Flags: D3D12_MULTISAMPLE_QUALITY_LEVELS_FLAG_NONE,
        NumQualityLevels: 0,
    };
    unsafe {
        check_feature::<D3D12_FEATURE_DATA_MULTISAMPLE_QUALITY_LEVELS>(
            device,
            D3D12_FEATURE_MULTISAMPLE_QUALITY_LEVELS,
            &mut features_architecture,
        )
    }?;
    println!("check_sample_support: {:?}", &features_architecture);
    // 在一切支持 Direct3D 11 的设备上，所有的渲染目标格式就皆已支持 4X MSAA 了。因此，凡是支持 Direct3D 11 的硬件，
    // 都会保证此项功能的正常开启，我们也就无须再对此进行检验了。但是，对质量级别的检测还是不可或缺
    assert!(features_architecture.NumQualityLevels > 0);
    Ok(features_architecture.NumQualityLevels)
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn check_feature<T>(
    device: &ID3D12Device,
    feature: D3D12_FEATURE,
    value: &mut T,
) -> Result<()> {
    device.CheckFeatureSupport(
        feature,
        value as *mut _ as *mut _,
        std::mem::size_of::<T>() as _,
    )
}

pub fn test(device: &ID3D12Device) {
    unsafe {
        let mut data = D3D12_FEATURE_DATA_MULTISAMPLE_QUALITY_LEVELS {
            SampleCount: 4,
            Format: DXGI_FORMAT_R32G32B32A32_UINT,
            Flags: D3D12_MULTISAMPLE_QUALITY_LEVELS_FLAG_NONE,
            NumQualityLevels: 0,
        };
        let result = device
            .CheckFeatureSupport(
                D3D12_FEATURE_MULTISAMPLE_QUALITY_LEVELS,
                &mut data as *mut _ as *mut _,
                std::mem::size_of::<D3D12_FEATURE_DATA_MULTISAMPLE_QUALITY_LEVELS>() as _,
            )
            .unwrap();
        println!("result {:?}", result);
        println!("data {:?}", data);
    }
}

/// 通常来讲，在绘制调用开始执行之前，我们应将不同的着色器程序所需的各种类型的资源绑定到渲染流水线上。
/// 实际上，不同类型的资源会被绑定到特定的寄存器槽（register slot）上，以供着色器程序访问。  
/// 根签名（root signature）定义的是：在执行绘制命令之前，那些应用程序将绑定到渲染流水线上的
/// 资源，它们会被映射到着色器的对应输入寄存器。根签名一定要与使用它的着色器相兼容（即在绘制开
/// 始之前，根签名一定要为着色器提供其执行期间需要绑定到渲染流水线的所有资源），在创建流水线状态
/// 对象（pipeline state object）时会对此进行验证（参见 6.9 节）。不同的绘制调用可能会用到一组不同的着
/// 色器程序，这也就意味着要用到不同的根签名。
/// 如果我们把着色器程序当作一个函数，而将输入资源看作着色器的函数参数，那么根签名则定义了函数签名
/// （其实这就是“根签名”一词的由来）。通过绑定不同的资源作为参数，着色器的输出也将有所差别。
/// 例如，顶点着色器的输出取决于实际向它输入的顶点数据以及为它绑定的具体资源。
pub fn create_root_signature(device: &ID3D12Device) -> Result<ID3D12RootSignature> {
    // 根签名由一组根参数构成
    let desc = D3D12_ROOT_SIGNATURE_DESC {
        Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        ..Default::default()
    };

    let mut signature = None;

    let signature = unsafe {
        D3D12SerializeRootSignature(
            &desc,
            D3D_ROOT_SIGNATURE_VERSION_1,
            &mut signature,
            std::ptr::null_mut(),
        )
    }
    .map(|()| signature.unwrap())?;

    // Direct3D 12 规定，必须先将根签名的描述布局进行序列化处理（serialize），待其转换为以 ID3DBlob 接口表示的序列化
    // 数据格式后，才可将它传入 CreateRootSignature 方法，正式创建根签名。
    unsafe {
        device.CreateRootSignature(
            0,
            std::slice::from_raw_parts(
                signature.GetBufferPointer() as _,
                signature.GetBufferSize(),
            ),
        )
    }
}

/// ID3D12PipelineState 对象集合了大量的流水线状态信息。为了保证性能，我们将所有这些对
/// 象都集总在一起，一并送至渲染流水线。通过这样的一个集合，Direct3D 便可以确定所有的状态是否彼
/// 此兼容，而驱动程序则能够据此而提前生成硬件本地指令及其状态。在 Direct3D 11 的状态模型中，这些
/// 渲染状态片段都是要分开配置的。然而这些状态实际都有一定的联系，以致如果其中的一个状态发生改
/// 变，那么驱动程序可能就要为了另一个相关的独立状态而对硬件重新进行编程。由于一些状态在配置流
/// 水线时需要改变，因而硬件状态也就可能被频繁地改写。为了避免这些冗余的操作，驱动程序往往会推
/// 迟针对硬件状态的编程动作，直到明确整条流水线的状态发起绘制调用后，才正式生成对应的本地指令
/// 与状态。但是，这种延迟操作需要驱动在运行时进行额外的记录工作，即追踪状态的变化，而后才能在
/// 运行时生成改写硬件状态的本地代码。在 Direct3D 12 的新模型中，驱动程序可以在初始化期间生成对流
/// 水线状态编程的全部代码，这便是我们将大多数的流水线状态指定为一个集合所带来的好处。
pub fn create_pipeline_state(
    device: &ID3D12Device,
    root_signature: &ID3D12RootSignature,
) -> Result<ID3D12PipelineState> {
    let compile_flags = if cfg!(debug_assertions) {
        D3DCOMPILE_DEBUG | D3DCOMPILE_SKIP_OPTIMIZATION
    } else {
        0
    };

    let exe_path = std::env::current_exe().ok().unwrap();
    let asset_path = exe_path.parent().unwrap();
    let shaders_hlsl_path = asset_path.join("shaders.hlsl");
    let shaders_hlsl = shaders_hlsl_path.to_str().unwrap();

    let mut vertex_shader = None;
    let vertex_shader = unsafe {
        D3DCompileFromFile(
            shaders_hlsl,
            std::ptr::null(),
            None,
            "VSMain",
            "vs_5_0",
            compile_flags,
            0,
            &mut vertex_shader,
            std::ptr::null_mut(),
        )
    }
    .map(|()| vertex_shader.unwrap())?;

    let mut pixel_shader = None;
    let pixel_shader = unsafe {
        D3DCompileFromFile(
            shaders_hlsl,
            std::ptr::null(),
            None,
            "PSMain",
            "ps_5_0",
            compile_flags,
            0,
            &mut pixel_shader,
            std::ptr::null_mut(),
        )
    }
    .map(|()| pixel_shader.unwrap())?;

    let mut input_element_descs: [D3D12_INPUT_ELEMENT_DESC; 2] = [
        D3D12_INPUT_ELEMENT_DESC {
            SemanticName: PCSTR(b"POSITION\0".as_ptr()),
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32B32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 0,
            InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        },
        D3D12_INPUT_ELEMENT_DESC {
            SemanticName: PCSTR(b"COLOR\0".as_ptr()),
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 12,
            InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        },
    ];

    // 大多数控制图形流水线状态的对象被统称为流水线状态对象（Pipeline State Object，PSO），用 ID3D12PipelineState 接口来表示。
    // 要创建 PSO，我们首先要填写一份描述其细节的 D3D12_GRAPHICS_PIPELINE_STATE_DESC 结构体实例。
    let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        InputLayout: D3D12_INPUT_LAYOUT_DESC {
            pInputElementDescs: input_element_descs.as_mut_ptr(),
            NumElements: input_element_descs.len() as u32,
        },
        // 指向一个与此 PSO 相绑定的根签名的指针。该根签名一定要与此 PSO 指定的着色器相兼容。
        pRootSignature: Some(root_signature.clone()),
        // 待绑定的顶点着色器。此成员由结构体 D3D12_SHADER_BYTECODE 表示，这个结构体存
        // 有指向已编译好的字节码数据的指针，以及该字节码数据所占的字节大小。
        VS: D3D12_SHADER_BYTECODE {
            pShaderBytecode: unsafe { vertex_shader.GetBufferPointer() },
            BytecodeLength: unsafe { vertex_shader.GetBufferSize() },
        },
        // 待绑定的像素着色器
        PS: D3D12_SHADER_BYTECODE {
            pShaderBytecode: unsafe { pixel_shader.GetBufferPointer() },
            BytecodeLength: unsafe { pixel_shader.GetBufferSize() },
        },
        // 指定用来配置光栅器的光栅化状态。
        RasterizerState: D3D12_RASTERIZER_DESC {
            FillMode: D3D12_FILL_MODE_SOLID,
            CullMode: D3D12_CULL_MODE_NONE,
            ..Default::default()
        },
        // 指定混合（blending）操作所用的混合状态。
        BlendState: D3D12_BLEND_DESC {
            AlphaToCoverageEnable: false.into(),
            IndependentBlendEnable: false.into(),
            RenderTarget: [
                D3D12_RENDER_TARGET_BLEND_DESC {
                    BlendEnable: false.into(),
                    LogicOpEnable: false.into(),
                    SrcBlend: D3D12_BLEND_ONE,
                    DestBlend: D3D12_BLEND_ZERO,
                    BlendOp: D3D12_BLEND_OP_ADD,
                    SrcBlendAlpha: D3D12_BLEND_ONE,
                    DestBlendAlpha: D3D12_BLEND_ZERO,
                    BlendOpAlpha: D3D12_BLEND_OP_ADD,
                    LogicOp: D3D12_LOGIC_OP_NOOP,
                    RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
                },
                D3D12_RENDER_TARGET_BLEND_DESC::default(),
                D3D12_RENDER_TARGET_BLEND_DESC::default(),
                D3D12_RENDER_TARGET_BLEND_DESC::default(),
                D3D12_RENDER_TARGET_BLEND_DESC::default(),
                D3D12_RENDER_TARGET_BLEND_DESC::default(),
                D3D12_RENDER_TARGET_BLEND_DESC::default(),
                D3D12_RENDER_TARGET_BLEND_DESC::default(),
            ],
        },
        // 指定用于配置深度/模板测试的深度/模板状态。
        DepthStencilState: D3D12_DEPTH_STENCIL_DESC::default(),
        // 多重采样最多可采集 32 个样本。借此参数的 32 位整数值，即可设置每个采样点的采集情况（采集或禁止采集）。
        // 例如，若禁用了第 5 位（将第 5 位设置为 0），则将不会对第 5 个样本进行采样。当然，要禁止采集第 5 个样本的前提是，
        // 所用的多重采样至少要有 5个样本。假如一个应用程序仅使用了单采样（single sampling），那么只能针对该参数的第 1 位
        // 进行配置。一般来说，使用的都是默认值 0xffffffff，即表示对所有的采样点都进行采样。
        SampleMask: u32::MAX,
        // 指定图元的拓扑类型。
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        // 同时所用的渲染目标数量（即 RTVFormats 数组中渲染目标格式的数量）。
        NumRenderTargets: 1,
        // 描述多重采样对每个像素采样的数量及其质量级别。此参数应与渲染目标的对应设置相匹配。
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    // 渲染目标的格式。利用该数组实现向多渲染目标同时进行写操作。使用此 PSO 的渲染目标的格式设定应当与此参数相匹配。
    desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;

    unsafe { device.CreateGraphicsPipelineState(&desc) }
}
