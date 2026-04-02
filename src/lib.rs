use std::ffi::c_void;
use std::sync::LazyLock;

use tracing::{debug, error, trace, warn};
use windows::core::{w, Interface, BOOL, HRESULT};
use windows::Win32::Foundation::{HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_REFERENCE, D3D_DRIVER_TYPE_WARP,
    D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0,
    D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11ClassLinkage, ID3D11DepthStencilView, ID3D11Device,
    ID3D11DeviceContext, ID3D11PixelShader, ID3D11RenderTargetView, ID3D11Resource,
    ID3D11ShaderResourceView, ID3D11Texture2D, ID3D11VertexShader, D3D11_BOX,
    D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION, D3D11_SHADER_RESOURCE_VIEW_DESC,
    D3D11_SUBRESOURCE_DATA, D3D11_TEXTURE2D_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    Common::{DXGI_FORMAT, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_MODE_DESC, DXGI_SAMPLE_DESC},
    CreateDXGIFactory1, IDXGIFactory1, IDXGISwapChain, DXGI_SWAP_CHAIN_DESC,
    DXGI_SWAP_EFFECT_DISCARD, DXGI_USAGE_RENDER_TARGET_OUTPUT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, RegisterClassExW, UnregisterClassW, WINDOW_EX_STYLE,
    WNDCLASSEXW, WS_OVERLAPPED,
};

use retour::GenericDetour;

pub struct Ocular {
    swap_chain: IDXGISwapChain,
    device: ID3D11Device,
    device_context: ID3D11DeviceContext,
}

impl Ocular {
    fn new() -> Ocular {
        // Create our own dummy window to avoid issues with GetForegroundWindow
        // which can return windows from other processes or protected system windows
        let hwnd = Self::create_dummy_window();

        let sd = DXGI_SWAP_CHAIN_DESC {
            BufferCount: 1,
            BufferDesc: DXGI_MODE_DESC {
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                Width: 1,
                Height: 1,
                ..Default::default()
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            OutputWindow: hwnd,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Windowed: BOOL(1),
            SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
            ..Default::default()
        };

        // Try multiple driver types for maximum compatibility
        let driver_types = [
            D3D_DRIVER_TYPE_HARDWARE,
            D3D_DRIVER_TYPE_WARP,
            D3D_DRIVER_TYPE_REFERENCE,
        ];

        // Feature levels to try, in descending order
        let feature_levels = [
            D3D_FEATURE_LEVEL_11_1,
            D3D_FEATURE_LEVEL_11_0,
            D3D_FEATURE_LEVEL_10_1,
            D3D_FEATURE_LEVEL_10_0,
        ];

        let mut p_device: Option<ID3D11Device> = None;
        let mut p_device_context: Option<ID3D11DeviceContext> = None;
        let mut last_error = None;

        // Try each driver type until one succeeds
        for driver_type in driver_types {
            trace!("Trying D3D11CreateDevice with {:?}...", driver_type);

            let res = unsafe {
                D3D11CreateDevice(
                    None,
                    driver_type,
                    HMODULE(std::ptr::null_mut()),
                    D3D11_CREATE_DEVICE_FLAG(0),
                    Some(&feature_levels),
                    D3D11_SDK_VERSION,
                    Some(&mut p_device),
                    None,
                    Some(&mut p_device_context),
                )
            };

            match res {
                Ok(_) if p_device.is_some() => {
                    trace!("D3D11CreateDevice succeeded with {:?}", driver_type);
                    break;
                }
                Err(e) => {
                    warn!("D3D11CreateDevice failed with {:?}: {:?}", driver_type, e);
                    last_error = Some(e);
                    p_device = None;
                    p_device_context = None;
                }
                _ => {}
            }
        }

        let p_device = match p_device {
            Some(device) => device,
            None => {
                Self::cleanup_dummy_window(hwnd);
                error!(
                    "All D3D11CreateDevice attempts failed: {:?}",
                    last_error
                );
                panic!(
                    "All D3D11CreateDevice attempts failed: {:?}",
                    last_error
                );
            }
        };

        trace!("Calling CreateDXGIFactory1...");
        let dxgi: IDXGIFactory1 = match unsafe { CreateDXGIFactory1() } {
            Ok(dxgi) => dxgi,
            Err(e) => {
                Self::cleanup_dummy_window(hwnd);
                error!("CreateDXGIFactory1 failed: {:?}", e);
                panic!("CreateDXGIFactory1 failed: {:?}", e);
            }
        };

        let mut p_swap_chain: Option<IDXGISwapChain> = None;

        trace!("Calling IDXGIFactory::CreateSwapChain...");
        let res = unsafe { dxgi.CreateSwapChain(&p_device, &sd, &mut p_swap_chain) };
        if res.is_err() {
            Self::cleanup_dummy_window(hwnd);
            error!("IDXGIFactory::CreateSwapChain failed: {:?}", res);
            panic!("IDXGIFactory::CreateSwapChain failed: {:?}", res);
        }

        // Clean up the dummy window after swap chain is created
        Self::cleanup_dummy_window(hwnd);

        Ocular {
            swap_chain: p_swap_chain.expect("No SwapChain found"),
            device: p_device,
            device_context: p_device_context.expect("No DeviceContext found"),
        }
    }

    /// Creates a hidden dummy window owned by our process for swap chain creation.
    /// This avoids issues with GetForegroundWindow() which can return:
    /// - Windows from other processes (access denied)
    /// - Protected/system windows
    /// - NULL if no window is focused
    fn create_dummy_window() -> HWND {
        const CLASS_NAME: windows::core::PCWSTR = w!("OcularDummyWindow");

        // Minimal window procedure
        unsafe extern "system" fn wnd_proc(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            windows::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        unsafe {
            let hmodule = GetModuleHandleW(None).unwrap_or_default();
            let hinstance: HINSTANCE = std::mem::transmute(hmodule);

            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wnd_proc),
                hInstance: hinstance,
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };

            // Register class (ignore error if already registered)
            RegisterClassExW(&wc);

            let hwnd = match CreateWindowExW(
                WINDOW_EX_STYLE(0),
                CLASS_NAME,
                w!(""),
                WS_OVERLAPPED,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(hinstance),
                None,
            ) {
                Ok(hwnd) => hwnd,
                Err(e) => {
                    error!("Failed to create dummy window: {:?}", e);
                    panic!("Failed to create dummy window: {:?}", e);
                }
            };

            trace!("Created dummy window: {:?}", hwnd);
            hwnd
        }
    }

    /// Cleans up the dummy window after use
    fn cleanup_dummy_window(hwnd: HWND) {
        const CLASS_NAME: windows::core::PCWSTR = w!("OcularDummyWindow");

        unsafe {
            let _ = DestroyWindow(hwnd);
            let hmodule = GetModuleHandleW(None).unwrap_or_default();
            let hinstance: HINSTANCE = std::mem::transmute(hmodule);
            let _ = UnregisterClassW(CLASS_NAME, Some(hinstance));
            trace!("Cleaned up dummy window");
        }
    }
}

static OCULAR: LazyLock<Ocular> = LazyLock::new(|| {
    debug!("Ocular not initialized, creating...");
    let ocular = Ocular::new();
    debug!("Ocular initialized!");
    ocular
});

pub fn get_ocular() -> &'static Ocular {
    &OCULAR
}

// SwapChain
type PresentFn =
    extern "system" fn(this: *mut IDXGISwapChain, sync_interval: u32, flags: u32) -> HRESULT;

type ResizeBuffersFn = extern "system" fn(
    this: *mut IDXGISwapChain,
    buffer_count: u32,
    width: u32,
    height: u32,
    new_format: DXGI_FORMAT,
    swap_chain_flags: u32,
) -> HRESULT;

type ResizeTargetFn = extern "system" fn(
    this: *mut IDXGISwapChain,
    p_new_target_parameters: *const DXGI_MODE_DESC,
) -> HRESULT;

// Device
type CreateVertexShaderFn = extern "system" fn(
    this: *mut ID3D11Device,
    p_shader_byte_code: *const c_void,
    p_class_linkage: Option<ID3D11ClassLinkage>,
    p_p_vertex_shader: *mut Option<ID3D11VertexShader>,
) -> HRESULT;

type CreatePixelShaderFn = extern "system" fn(
    this: *mut ID3D11Device,
    p_shader_byte_code: *const c_void,
    p_class_linkage: Option<ID3D11ClassLinkage>,
    p_p_pixel_shader: *mut Option<ID3D11PixelShader>,
) -> HRESULT;

type CreateTexture2DFn = extern "system" fn(
    this: *mut ID3D11Device,
    p_desc: *const D3D11_TEXTURE2D_DESC,
    p_initial_data: *const D3D11_SUBRESOURCE_DATA,
    pp_texture_2d: *mut *mut ID3D11Texture2D,
) -> HRESULT;

type CreateShaderResourceViewFn = extern "system" fn(
    this: *mut ID3D11Device,
    p_resource: *mut ID3D11Resource,
    p_desc: *const D3D11_SHADER_RESOURCE_VIEW_DESC,
    pp_srv: *mut *mut ID3D11ShaderResourceView,
) -> HRESULT;

// Device Context
type OMSetRenderTargetsFn = extern "system" fn(
    this: *mut ID3D11DeviceContext,
    num_views: u32,
    render_target_views: *const Option<ID3D11RenderTargetView>,
    depth_stencil_view: Option<ID3D11DepthStencilView>,
);

type UpdateSubresourceFn = extern "system" fn(
    this: *mut ID3D11DeviceContext,
    p_dst_resource: *mut ID3D11Resource,
    dst_subresource: u32,
    p_dst_box: *const D3D11_BOX,
    p_src_data: *const c_void,
    src_row_pitch: u32,
    src_depth_pitch: u32,
);

type CopyResourceFn = extern "system" fn(
    this: *mut ID3D11DeviceContext,
    p_dst_resource: *mut ID3D11Resource,
    p_src_resource: *mut ID3D11Resource,
);

type PSSetShaderResourcesFn = extern "system" fn(
    this: *mut ID3D11DeviceContext,
    start_slot: u32,
    num_views: u32,
    pp_shader_resource_views: *const *mut ID3D11ShaderResourceView,
);

use hook_macro::Hookable;

#[allow(dead_code)]
#[derive(Hookable)]
enum SwapChain {
    Present,
    ResizeBuffers,
    ResizeTarget,
}

#[allow(dead_code)]
#[derive(Hookable)]
enum Device {
    CreateVertexShader,
    CreatePixelShader,
    CreateTexture2D,
    CreateShaderResourceView,
}

#[allow(dead_code)]
#[derive(Hookable)]
enum DeviceContext {
    OMSetRenderTargets,
    UpdateSubresource,
    CopyResource,
    PSSetShaderResources,
}
