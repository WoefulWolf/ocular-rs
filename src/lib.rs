use std::ffi::c_void;
use std::sync::LazyLock;

use tracing::{debug, error, trace};
use windows::core::{Interface, BOOL, HRESULT};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11ClassLinkage, ID3D11DepthStencilView, ID3D11Device,
    ID3D11DeviceContext, ID3D11PixelShader, ID3D11RenderTargetView, ID3D11Resource,
    ID3D11ShaderResourceView, ID3D11Texture2D, ID3D11VertexShader, D3D11_BOX,
    D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION, D3D11_SHADER_RESOURCE_VIEW_DESC,
    D3D11_SUBRESOURCE_DATA, D3D11_TEXTURE2D_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    Common::{DXGI_FORMAT, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_MODE_DESC, DXGI_SAMPLE_DESC},
    IDXGISwapChain, DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_EFFECT_DISCARD,
    DXGI_USAGE_RENDER_TARGET_OUTPUT,
};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory, IDXGIFactory};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

use retour::GenericDetour;

pub struct Ocular {
    swap_chain: IDXGISwapChain,
    device: ID3D11Device,
    device_context: ID3D11DeviceContext,
}

impl Ocular {
    fn new() -> Ocular {
        let sd: *const DXGI_SWAP_CHAIN_DESC = &DXGI_SWAP_CHAIN_DESC {
            BufferCount: 1,
            BufferDesc: DXGI_MODE_DESC {
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                ..Default::default()
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            OutputWindow: unsafe { GetForegroundWindow() },
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
                ..Default::default()
            },
            Windowed: BOOL(1),
            SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
            ..Default::default()
        };

        let mut p_swap_chain: Option<IDXGISwapChain> = None;
        let mut p_device: Option<ID3D11Device> = None;
        let mut p_device_context: Option<ID3D11DeviceContext> = None;

        trace!("Calling D3D11CreateDevice...");
        let res = unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE(0 as *mut c_void),
                D3D11_CREATE_DEVICE_FLAG(0),
                None,
                D3D11_SDK_VERSION,
                Some(&mut p_device),
                Some(std::ptr::null_mut()),
                Some(&mut p_device_context),
            )
        };

        match res {
            Ok(_) => {}
            Err(e) => {
                error!("D3D11CreateDevice failed: {:?}", e);
                panic!("D3D11CreateDevice failed: {:?}", e);
            }
        }

        trace!("Calling CreateDXGIFactory...");
        let dxgi: IDXGIFactory = match unsafe { CreateDXGIFactory() } {
            Ok(dxgi) => dxgi,
            Err(e) => {
                error!("CreateDXGIFactory failed: {:?}", e);
                panic!("CreateDXGIFactory failed: {:?}", e);
            }
        };

        trace!("Calling IDXGIFactory::CreateSwapChain...");
        let res = unsafe {
            dxgi.CreateSwapChain(
                p_device.as_ref().expect("pDevice was None"),
                sd,
                &mut p_swap_chain,
            )
            .ok()
        };

        match res {
            Ok(_) => {}
            Err(e) => {
                error!("IDXGIFactory::CreateSwapChain failed: {:?}", e);
                panic!("IDXGIFactory::CreateSwapChain failed: {:?}", e);
            }
        }

        Ocular {
            swap_chain: p_swap_chain.expect("No SwapChain found"),
            device: p_device.expect("No Device found"),
            device_context: p_device_context.expect("No DeviceContext found"),
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
}
