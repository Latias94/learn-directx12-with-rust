use crate::SampleCommandLine;
use std::mem::transmute;
use windows::Win32::Graphics::Gdi::UpdateWindow;
use windows::{
    core::*, Win32::Foundation::*, Win32::System::LibraryLoader::*,
    Win32::UI::WindowsAndMessaging::*,
};

pub trait DXSample {
    fn new(command_line: &SampleCommandLine) -> Result<Self>
    where
        Self: Sized;
    fn bind_to_window(&mut self, hwnd: &HWND) -> Result<()>;
    fn update(&mut self) {}
    fn render(&mut self);
    fn on_key_up(&mut self, _key: u8) {}
    fn on_key_down(&mut self, _key: u8) {}

    fn title(&self) -> String {
        "DXSample".into()
    }

    fn window_size(&self) -> (i32, i32) {
        (1024, 768)
    }
}

pub fn init_sample<S: DXSample>() -> Result<()> {
    let instance = unsafe { GetModuleHandleA(None) }.unwrap();
    debug_assert!(!instance.is_invalid());
    // // 第一项任务便是通过填写 WNDCLASS 结构体，并根据其中描述的特征来创建一个窗口
    let wc = WNDCLASSEXA {
        cbSize: std::mem::size_of::<WNDCLASSEXA>() as u32,
        // CS_HREDRAW: 如果移动或大小调整更改了工作区的宽度，将重绘整个窗口。
        // CS_VREDRAW: 如果移动或大小调整更改了工作区的高度，将重绘整个窗口。
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc::<S>),
        hInstance: instance,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        lpszClassName: PCSTR(b"RustWindowClass\0".as_ptr()),
        ..Default::default()
    };
    let command_line = SampleCommandLine::default();
    let mut sample = S::new(&command_line)?;
    let size = sample.window_size();
    // 我们要在 Windows 系统中为上述 WNDCLASS 注册一个实例，这样一来，即可据此创建窗口。
    let atom = unsafe { RegisterClassExA(&wc) };
    debug_assert_ne!(atom, 0);

    let window_rect = RECT {
        left: 0,
        top: 0,
        right: size.0,
        bottom: size.1,
    };
    // unsafe { AdjustWindowRect(&mut window_rect, WS_OVERLAPPEDWINDOW, false) };
    let mut title = sample.title();

    if command_line.use_warp_device {
        title.push_str(" (WARP)");
    }
    let hwnd = unsafe {
        CreateWindowExA(
            Default::default(),
            s!("RustWindowClass"), // 创建此窗口采用的是前面注册的 WNDCLASS 实例
            PCSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW,                  // 窗口的样式标志
            CW_USEDEFAULT,                        // x 坐标
            CW_USEDEFAULT,                        // y 坐标
            window_rect.right - window_rect.left, // 窗口宽度
            window_rect.bottom - window_rect.top, // 窗口高度
            None,                                 // no parent window
            None,                                 // no menus
            instance,                             // 应用程序实例句柄
            Some(&mut sample as *mut _ as _),     // 可在此设置一些创建窗口所用的其他参数
        )
    };

    sample.bind_to_window(&hwnd)?;

    // 尽管窗口已经创建完毕，但仍没有显示出来。因此，最后一步便是调用下面的两个函数，将刚刚创建的窗口展示出来
    // 并对它进行更新。可以看出，我们为这两个函数都传入了窗口句柄，这样一来，它们就知道需要展示以及更新的窗口是哪一个
    unsafe { ShowWindow(hwnd, SW_SHOW) };
    unsafe { UpdateWindow(hwnd) };

    loop {
        let mut message = MSG::default();
        // 在获取 WM_QUIT 消息之前，该函数会一直保持循环。GetMessage 函数只有在收到 WM_QUIT 消
        // 息时才会返回 0（false），这会造成循环终止；而若发生错误，它便会返回-1。还需注意的一点是，
        // 在未有信息到来之时，GetMessage 函数会令此应用程序线程进入休眠状态
        if unsafe { PeekMessageA(&mut message, None, 0, 0, PM_REMOVE) }.into() {
            unsafe {
                TranslateMessage(&message);
                DispatchMessageA(&message);
            }

            if message.message == WM_QUIT {
                break;
            }
        }
    }
    Ok(())
}

/// 窗口过程会处理窗口所接收到的消息
fn sample_wndproc<S: DXSample>(sample: &mut S, message: u32, wparam: WPARAM) -> bool {
    match message {
        WM_KEYDOWN => {
            sample.on_key_down(wparam.0 as u8);
            true
        }
        WM_KEYUP => {
            sample.on_key_up(wparam.0 as u8);
            true
        }
        WM_PAINT => {
            sample.update();
            sample.render();
            true
        }
        _ => false,
    }
}

#[allow(non_snake_case)]
#[cfg(target_pointer_width = "32")]
unsafe fn SetWindowLong(window: HWND, index: WINDOW_LONG_PTR_INDEX, value: isize) -> isize {
    SetWindowLongA(window, index, value as _) as _
}

#[allow(non_snake_case)]
#[cfg(target_pointer_width = "64")]
unsafe fn SetWindowLong(window: HWND, index: WINDOW_LONG_PTR_INDEX, value: isize) -> isize {
    SetWindowLongPtrA(window, index, value)
}

#[allow(non_snake_case)]
#[cfg(target_pointer_width = "32")]
unsafe fn GetWindowLong(window: HWND, index: WINDOW_LONG_PTR_INDEX) -> isize {
    GetWindowLongA(window, index) as _
}

#[allow(non_snake_case)]
#[cfg(target_pointer_width = "64")]
unsafe fn GetWindowLong(window: HWND, index: WINDOW_LONG_PTR_INDEX) -> isize {
    GetWindowLongPtrA(window, index)
}

extern "system" fn wndproc<S: DXSample>(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_CREATE => {
            unsafe {
                let create_struct: &CREATESTRUCTA = transmute(lparam);
                SetWindowLong(window, GWLP_USERDATA, create_struct.lpCreateParams as _);
            }
            LRESULT::default()
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT::default()
        }
        _ => {
            let user_data = unsafe { GetWindowLong(window, GWLP_USERDATA) };
            let sample = std::ptr::NonNull::<S>::new(user_data as _);
            let handled = sample.map_or(false, |mut s| {
                sample_wndproc(unsafe { s.as_mut() }, message, wparam)
            });

            if handled {
                LRESULT::default()
            } else {
                unsafe { DefWindowProcA(window, message, wparam, lparam) }
            }
        }
    }
}
