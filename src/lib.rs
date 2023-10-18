use std::ffi::c_void;

use windows::core::{
    HRESULT,
    Interface
};
use windows::Win32::Foundation::BOOL;
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use windows::Win32::Graphics::Dxgi::{
    IDXGISwapChain,
    DXGI_SWAP_CHAIN_DESC,
    DXGI_USAGE_RENDER_TARGET_OUTPUT,
    DXGI_SWAP_EFFECT_DISCARD,
    Common::{
        DXGI_FORMAT,
        DXGI_MODE_DESC,
        DXGI_SAMPLE_DESC,
        DXGI_FORMAT_R8G8B8A8_UNORM,
    }
};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device,
    ID3D11DeviceContext,
    ID3D11RenderTargetView,
    ID3D11DepthStencilView,
    D3D11CreateDeviceAndSwapChain,
    D3D11_CREATE_DEVICE_FLAG,
    D3D11_SDK_VERSION, ID3D11ClassLinkage, ID3D11VertexShader, ID3D11PixelShader
};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;

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

        unsafe {
            D3D11CreateDeviceAndSwapChain(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_FLAG(0),
                None,
                D3D11_SDK_VERSION,
                Some(sd),
                Some(&mut p_swap_chain),
                Some(&mut p_device),
                Some(std::ptr::null_mut()),
                Some(&mut p_device_context),
                ).expect("D3D11CreateDeviceAndSwapChain failed");
        }

        Ocular {
            swap_chain: p_swap_chain.expect("No SwapChain found"),
            device: p_device.expect("No Device found"),
            device_context: p_device_context.expect("No DeviceContext found"),
        }
    }
}

static mut OCULAR: Option<Ocular> = None;
fn get_ocular() -> &'static Ocular {
    unsafe {
        if OCULAR.is_none() {
            OCULAR = Some(Ocular::new());
        }

        OCULAR.as_ref().unwrap()
    }
}

// SwapChain
type PresentFn = extern "system" fn(
    this: IDXGISwapChain,
    sync_interval: u32,
    flags: u32,
) -> HRESULT;

type ResizeBuffersFn = extern "system" fn(
    this: IDXGISwapChain,
    buffer_count: u32,
    width: u32,
    height: u32,
    new_format: DXGI_FORMAT,
    swap_chain_flags: u32,
) -> HRESULT;

type ResizeTargetFn = extern "system" fn(
    this: IDXGISwapChain,
    p_new_target_parameters: *const DXGI_MODE_DESC,
) -> HRESULT;

// Device
type CreateVertexShaderFn = extern "system" fn(
    this: ID3D11Device,
    p_shader_byte_code: *const c_void,
    p_class_linkage: Option<ID3D11ClassLinkage>,
    p_p_vertex_shader: *mut Option<ID3D11VertexShader>,
) -> HRESULT;

type CreatePixelShaderFn = extern "system" fn(
    this: ID3D11Device,
    p_shader_byte_code: *const c_void,
    p_class_linkage: Option<ID3D11ClassLinkage>,
    p_p_pixel_shader: *mut Option<ID3D11PixelShader>,   
) -> HRESULT;

// Device Context
type OMSetRenderTargetsFn = extern "system" fn(
    This: ID3D11DeviceContext,
    num_views: u32,
    render_target_views: *const Option<ID3D11RenderTargetView>,
    depth_stencil_view: Option<ID3D11DepthStencilView>,
);

use hook_macro::Hookable;

#[allow(dead_code)]
#[derive(Hookable)]
enum SwapChain {
    Present,
    ResizeBuffers,
    ResizeTarget
}

#[allow(dead_code)]
#[derive(Hookable)]
enum Device {
    CreateVertexShader,
    CreatePixelShader,
}

#[allow(dead_code)]
#[derive(Hookable)]
enum DeviceContext {
    OMSetRenderTargets
}

