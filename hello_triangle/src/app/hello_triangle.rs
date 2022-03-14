use crate::devices::{create_device, create_pipeline_state, create_root_signature};
use crate::{DXSample, SampleCommandLine};
use windows::{
    core::*, Win32::Foundation::*, Win32::Graphics::Direct3D::*, Win32::Graphics::Direct3D12::*,
    Win32::Graphics::Dxgi::Common::*, Win32::Graphics::Dxgi::*, Win32::System::Threading::*,
    Win32::System::WindowsProgramming::*,
};

const FRAME_COUNT: u32 = 2;

pub struct Sample {
    dxgi_factory: IDXGIFactory4,
    device: ID3D12Device,
    resources: Option<Resources>,
}

struct Resources {
    command_queue: ID3D12CommandQueue,
    swap_chain: IDXGISwapChain3,
    frame_index: u32,
    render_targets: [ID3D12Resource; FRAME_COUNT as usize],
    rtv_heap: ID3D12DescriptorHeap,
    rtv_descriptor_size: usize,
    viewport: D3D12_VIEWPORT,
    scissor_rect: RECT,
    command_allocator: ID3D12CommandAllocator,
    root_signature: ID3D12RootSignature,
    pso: ID3D12PipelineState,
    command_list: ID3D12GraphicsCommandList,

    // we need to keep this around to keep the reference alive, even though
    // nothing reads from it
    #[allow(dead_code)]
    vertex_buffer: ID3D12Resource,

    vbv: D3D12_VERTEX_BUFFER_VIEW,
    fence: ID3D12Fence,
    fence_value: u64,
    fence_event: HANDLE,
}

/// 1. 用 `D3D12CreateDevice` 函数创建 `ID3D12Device` 接口实例。
/// 2. 创建一个 `ID3D12Fence` 对象，并查询描述符的大小。
/// 3. 检测用户设备对 4X MSAA 质量级别的支持情况。
/// 4. 依次创建命令队列、命令列表分配器和主命令列表。
/// 5. 描述并创建交换链。
/// 6. 创建应用程序所需的描述符堆。
/// 7. 调整后台缓冲区的大小，并为它创建渲染目标视图。
/// 8. 创建深度/模板缓冲区及与之关联的深度/模板视图。
/// 9. 设置视口（viewport）和裁剪矩形（scissor rectangle）。
impl DXSample for Sample {
    fn new(command_line: &SampleCommandLine) -> Result<Self>
    where
        Self: Sized,
    {
        let (dxgi_factory, device) = create_device(command_line)?;
        Ok(Sample {
            dxgi_factory,
            device,
            resources: None,
        })
    }

    fn bind_to_window(&mut self, hwnd: &HWND) -> Result<()> {
        let command_queue: ID3D12CommandQueue = unsafe {
            self.device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                ..Default::default()
            })?
        };
        let (width, height) = self.window_size();

        let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
            // 交换链中所用的缓冲区数量。我们将它指定为2，即采用双缓冲。
            BufferCount: FRAME_COUNT,
            Width: width as u32,
            Height: height as u32,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            // 由于我们要将数据渲染至后台缓冲区（即用它作为渲染目标），因此将此参数指定为 DXGI_USAGE_RENDER_TARGET_OUTPUT。
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            // 一个 DXGI_SWAP_EFFECT 类型的值描述了交换链所使用的表现模式，以及在呈现一个 surface 后处理表现缓冲区内容的选项。
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            // 多重采样的质量级别以及对每个像素的采样次数，可参见 4.1.8 节。
            // 对于单次采样来说，我们要将采样数量指定为 1，质量级别指定为 0。
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                ..Default::default()
            },
            ..Default::default()
        };

        let swap_chain: IDXGISwapChain3 = unsafe {
            self.dxgi_factory.CreateSwapChainForHwnd(
                &command_queue,
                hwnd,
                &swap_chain_desc,
                std::ptr::null(),
                None,
            )?
        }
        .cast()?;

        // This sample does not support fullscreen transitions
        unsafe {
            self.dxgi_factory
                .MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER)?;
        }
        // 用来记录当前后台缓冲区的索引（由于利用页面翻转技术来交换前台缓冲区和后台缓冲区，
        // 所以我们需要对其进行记录，以便搞清楚哪个缓冲区才是当前正在用于渲染数据的后台缓冲区）。
        let frame_index = unsafe { swap_chain.GetCurrentBackBufferIndex() };

        // 我们将为交换链中 NumDescriptors 个用于渲染数据的缓冲区资源创建对应的渲染目标视图（Render Target View，RTV）
        let rtv_heap: ID3D12DescriptorHeap = unsafe {
            self.device
                .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                    NumDescriptors: FRAME_COUNT,
                    Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                    ..Default::default()
                })
        }?;
        let rtv_descriptor_size = unsafe {
            self.device
                .GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV)
        } as usize;
        // 创建描述符堆之后，还要能访问其中所存的描述符。在程序中，我们是通过句柄来引用描述符的，
        // 并以 ID3D12DescriptorHeap::GetCPUDescriptorHandleForHeapStart 方法来获得描述符堆中第一个描述符的句柄。
        let rtv_handle = unsafe { rtv_heap.GetCPUDescriptorHandleForHeapStart() };

        // 资源不能与渲染流水线中的阶段直接绑定，所以我们必须先为资源创建视图（描述符），并将其绑定到流水线阶段。
        // 例如，为了将后台缓冲区绑定到流水线的输出合并阶段（output merger stage，这样Direct3D才能向其渲染），
        // 便需要为该后台缓冲区创建一个渲染目标视图。而这第一个步骤就是要获得存于交换链中的缓冲区资源。
        let render_targets: [ID3D12Resource; FRAME_COUNT as usize] =
            array_init::try_array_init(|i: usize| -> Result<ID3D12Resource> {
                // i 是希望获得的特定后台缓冲区的索引（有时后台缓冲区并不只一个，所以需要用索引来指明）。
                let render_target: ID3D12Resource = unsafe { swap_chain.GetBuffer(i as u32) }?;
                unsafe {
                    // 为获取的后台缓冲区创建渲染目标视图
                    self.device.CreateRenderTargetView(
                        // 指定用作渲染目标的资源。这里是后台缓冲区（即为后台缓冲区创建了一个渲染目标视图）。
                        &render_target,
                        // 指向 D3D12_RENDER_TARGET_VIEW_DESC 数据结构实例的指针。该结构体描述了资源中元素的数据类型（格式）。
                        // 如果该资源在创建时已指定了具体格式（即此资源不是无类型格式，not typeless），那么就可以把这个参数设为空指针，
                        // 表示采用该资源创建时的格式，为它的第一个 mipmap 层级（后台缓冲区只有一种 mipmap 层级，
                        // 有关 mipmap 的内容将在第 9 章展开讨论）创建一个视图。由于已经指定了后台缓冲区的格式，因此就将这个参数设置为空指针。
                        std::ptr::null(),
                        // 引用所创建渲染目标视图的描述符句柄
                        &D3D12_CPU_DESCRIPTOR_HANDLE {
                            ptr: rtv_handle.ptr + i * rtv_descriptor_size,
                        },
                    )
                };
                Ok(render_target)
            })?;

        let viewport = D3D12_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: width as f32,
            Height: height as f32,
            MinDepth: D3D12_MIN_DEPTH,
            MaxDepth: D3D12_MAX_DEPTH,
        };

        let scissor_rect = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };

        let command_allocator = unsafe {
            self.device
                .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
        }?;

        let root_signature = create_root_signature(&self.device)?;

        let pso = create_pipeline_state(&self.device, &root_signature)?;
        let command_list: ID3D12GraphicsCommandList = unsafe {
            self.device.CreateCommandList(
                0,
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                &command_allocator,
                &pso,
            )
        }?;
        unsafe {
            command_list.Close()?;
        };

        let aspect_ratio = width as f32 / height as f32;

        let (vertex_buffer, vbv) = create_vertex_buffer(&self.device, aspect_ratio)?;

        let fence = unsafe { self.device.CreateFence(0, D3D12_FENCE_FLAG_NONE) }?;

        let fence_value = 1;

        let fence_event = unsafe { CreateEventA(std::ptr::null(), false, false, None) };

        self.resources = Some(Resources {
            command_queue,
            swap_chain,
            frame_index,
            render_targets,
            rtv_heap,
            rtv_descriptor_size,
            viewport,
            scissor_rect,
            command_allocator,
            root_signature,
            pso,
            command_list,
            vertex_buffer,
            vbv,
            fence,
            fence_value,
            fence_event,
        });

        Ok(())
    }

    fn update(&mut self) {}

    fn render(&mut self) {
        if let Some(resources) = &mut self.resources {
            populate_command_list(resources).unwrap();

            // Execute the command list.
            let command_list = ID3D12CommandList::from(&resources.command_list);

            unsafe {
                resources
                    .command_queue
                    .ExecuteCommandLists(1, &Some(command_list))
            };

            // Present the frame.
            unsafe { resources.swap_chain.Present(1, 0) }.ok().unwrap();
            wait_for_previous_frame(resources);
        }
    }
}

fn populate_command_list(resources: &Resources) -> Result<()> {
    // Command list allocators can only be reset when the associated
    // command lists have finished execution on the GPU; apps should use
    // fences to determine GPU execution progress.
    // 向 GPU 提交了一整帧的渲染命令后，我们可能还要为了绘制下一帧而复用命令分配器中的内存。
    // 由于命令队列可能会引用命令分配器中的数据，所以在没有确定 GPU 执行完命令分配器中的所有命令之前，千万不要重置命令分配器！
    unsafe {
        resources.command_allocator.Reset()?;
    }

    let command_list = &resources.command_list;

    // However, when ExecuteCommandList() is called on a particular
    // command list, that command list can then be reset at any time and
    // must be before re-recording.
    // 此方法将命令列表恢复为刚创建时的初始状态，我们可以借此继续复用其低层内存，也可以避免释放旧列表再创建新列表这一系列的烦琐操作。
    // 注意，重置命令列表并不会影响命令队列中的命令，因为相关的命令分配器仍在维护着其内存中被命令队列引用的系列命令。
    // 向 GPU 提交了一整帧的渲染命令后，我们可能还要为了绘制下一帧而复用命令分配器中的内存。
    unsafe {
        command_list.Reset(&resources.command_allocator, &resources.pso)?;
    }

    // Set necessary state.
    unsafe {
        // 将根签名设置到命令列表上
        command_list.SetGraphicsRootSignature(&resources.root_signature);
        // 设置一个视口，将场景绘至整个后台缓冲区
        // 第一个参数是要绑定的视口数量（有些高级效果需要使用多个视口），第二个参数是一个指向视口数组的指针。
        command_list.RSSetViewports(1, &resources.viewport);
        // 设置裁剪矩形。
        // 类似于 RSSetViewports 方法，RSSetScissorRects 方法的第一个参数是要绑定的裁剪矩形数
        // 量（为了实现一些高级效果有时会采用多个裁剪矩形），第二个参数是指向一个裁剪矩形数组的指针。
        command_list.RSSetScissorRects(1, &resources.scissor_rect);
    }

    // Indicate that the back buffer will be used as a render target.
    // 这段代码将以图片形式显示在屏幕中的纹理，从呈现状态转换为渲染目标状态。
    let barrier = transition_barrier(
        &resources.render_targets[resources.frame_index as usize],
        D3D12_RESOURCE_STATE_PRESENT,
        D3D12_RESOURCE_STATE_RENDER_TARGET,
    );
    unsafe { command_list.ResourceBarrier(1, &barrier) };

    // 从描述符堆中获取描述符
    let rtv_handle = D3D12_CPU_DESCRIPTOR_HANDLE {
        // 在程序中，我们是通过句柄来引用描述符的
        // 下面通过 GetCPUDescriptorHandleForHeapStart 方法来获得描述符堆中第一个描述符的句柄
        ptr: unsafe { resources.rtv_heap.GetCPUDescriptorHandleForHeapStart() }.ptr
            + resources.frame_index as usize * resources.rtv_descriptor_size,
    };
    // 指定将要渲染的缓冲区
    unsafe { command_list.OMSetRenderTargets(1, &rtv_handle, false, std::ptr::null()) };

    // Record commands.
    unsafe {
        // 清除后台缓冲区
        command_list.ClearRenderTargetView(
            rtv_handle,
            [0.0, 0.2, 0.4, 1.0].as_ptr(),
            0,
            std::ptr::null(),
        );
        command_list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        // 在顶点缓冲区及其对应视图创建完成后，便可以将它与渲染流水线上的一个输入槽（input slot）相绑定。
        // 这样一来，我们就能向流水线中的输入装配器阶段传递顶点数据了。
        command_list.IASetVertexBuffers(0, 1, &resources.vbv);
        // 将顶点缓冲区设置到输入槽上并不会对其执行实际的绘制操作，而是仅为顶点数据送至渲染流水线做好准备而已。
        // 这最后一步才是通过 ID3D12GraphicsCommandList::DrawInstanced 方法真正地绘制顶点。
        // 1. VertexCountPerInstance：每个实例要绘制的顶点数量。
        // 2. InstanceCount：用于实现一种被称作实例化（instancing）的高级技术。就目前来说，我们只绘制一个实例，因而将此参数设置为 1。
        // 3. StartVertexLocation：指定顶点缓冲区内第一个被绘制顶点的索引（该索引值以 0 为基准）。
        // 4. StartInstanceLocation：用于实现一种被称作实例化的高级技术，暂时只需将其设置为 0。
        // VertexCountPerInstance 和 StartVertexLocation 两个参数定义了顶点缓冲区中将要被绘制的一组连续顶点，
        command_list.DrawInstanced(3, 1, 0, 0);

        // Indicate that the back buffer will now be used to present.
        command_list.ResourceBarrier(
            1,
            &transition_barrier(
                &resources.render_targets[resources.frame_index as usize],
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_PRESENT,
            ),
        );
    }

    unsafe { command_list.Close() }
}

/// 通过命令列表设置转换资源屏障（transition resource barrier）数组，即可指定资源的转换；当我们希
/// 望以一次 API 调用来转换多个资源的时候，这种数组就派上了用场。
/// 我们可以将此资源屏障转换看作是一条告知 GPU 某资源状态正在进行转换的命令。所以在执行后续的命令时，GPU 便会采取必要措施以防资源冒险。
fn transition_barrier(
    resource: &ID3D12Resource,
    state_before: D3D12_RESOURCE_STATES,
    state_after: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: std::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: Some(resource.clone()),
                StateBefore: state_before,
                StateAfter: state_after,
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
            }),
        },
    }
}

#[repr(C)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
}

fn create_vertex_buffer(
    device: &ID3D12Device,
    aspect_ratio: f32,
) -> Result<(ID3D12Resource, D3D12_VERTEX_BUFFER_VIEW)> {
    let vertices = [
        Vertex {
            position: [0.0, 0.25 * aspect_ratio, 0.0],
            color: [1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [0.25, -0.25 * aspect_ratio, 0.0],
            color: [0.0, 1.0, 0.0, 1.0],
        },
        Vertex {
            position: [-0.25, -0.25 * aspect_ratio, 0.0],
            color: [0.0, 0.0, 1.0, 1.0],
        },
    ];

    // Note: using upload heaps to transfer static data like vert buffers is
    // not recommended. Every time the GPU needs it, the upload heap will be
    // marshalled over. Please read up on Default Heap usage. An upload heap
    // is used here for code simplicity and because there are very few verts
    // to actually transfer.
    let mut vertex_buffer: Option<ID3D12Resource> = None;
    unsafe {
        // GPU 资源都存于堆（heap）中，其本质是具有特定属性的 GPU 显存块。ID3D12Device::
        // CreateCommittedResource 方法将根据我们所提供的属性创建一个资源与一个堆，并把该资源提交到这个堆中。
        device.CreateCommittedResource(
            &D3D12_HEAP_PROPERTIES {
                Type: D3D12_HEAP_TYPE_UPLOAD,
                ..Default::default()
            },
            D3D12_HEAP_FLAG_NONE,
            &D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                Width: std::mem::size_of_val(&vertices) as u64,
                Height: 1,
                DepthOrArraySize: 1,
                MipLevels: 1,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                ..Default::default()
            },
            D3D12_RESOURCE_STATE_GENERIC_READ,
            std::ptr::null(),
            &mut vertex_buffer,
        )?
    };
    let vertex_buffer = vertex_buffer.unwrap();

    // Copy the triangle data to the vertex buffer.
    unsafe {
        let mut data = std::ptr::null_mut();
        vertex_buffer.Map(0, std::ptr::null(), &mut data)?;
        std::ptr::copy_nonoverlapping(
            vertices.as_ptr(),
            data as *mut Vertex,
            std::mem::size_of_val(&vertices),
        );
        vertex_buffer.Unmap(0, std::ptr::null());
    }

    let vbv = D3D12_VERTEX_BUFFER_VIEW {
        BufferLocation: unsafe { vertex_buffer.GetGPUVirtualAddress() },
        StrideInBytes: std::mem::size_of::<Vertex>() as u32,
        SizeInBytes: std::mem::size_of_val(&vertices) as u32,
    };

    Ok((vertex_buffer, vbv))
}

fn wait_for_previous_frame(resources: &mut Resources) {
    // WAITING FOR THE FRAME TO COMPLETE BEFORE CONTINUING IS NOT BEST
    // PRACTICE. This is code implemented as such for simplicity. The
    // D3D12HelloFrameBuffering sample illustrates how to use fences for
    // efficient resource usage and to maximize GPU utilization.

    // Signal and increment the fence value.
    let fence = resources.fence_value;
    // 向命令队列中添加一条用来设置新围栏点的命令。
    // 由于这条命令要交由 GPU 处理（即由 GPU 端来修改围栏值），
    // 所以在 GPU 处理完命令队列中此 Signal() 以前的所有命令之前，它并不会设置新的围栏点
    unsafe { resources.command_queue.Signal(&resources.fence, fence) }
        .ok()
        .unwrap();
    // 增加围栏值
    resources.fence_value += 1;

    // 在 CPU 端等待 GPU，直到后者执行完这个围栏点之前的所有命令
    if unsafe { resources.fence.GetCompletedValue() } < fence {
        // 若 GPU 命中当前的围栏（即执行到 Signal()指令，修改了围栏值），则激发预定事件
        unsafe {
            resources
                .fence
                .SetEventOnCompletion(fence, resources.fence_event)
        }
        .ok()
        .unwrap();

        // 等待 GPU 命中围栏，激发事件
        unsafe { WaitForSingleObject(resources.fence_event, INFINITE) };
    }

    resources.frame_index = unsafe { resources.swap_chain.GetCurrentBackBufferIndex() };
}
