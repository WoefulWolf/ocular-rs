<h1 align="center">
🦀🔭 ocular-rs 🌠🦀 
</h1>

<h3 align="center">
A Simple DX11 Hooking Library using <a href="https://github.com/Hpmason/retour-rs">retours-rs</a>
</h3>
<p align = "center">
Rust "port" of my <a href = "https://github.com/WoefulWolf/ocular">C++ library.</a>
</p>

## Example Usage
```rust
// Just some windows-rs stuff to declare our DLL entry point.
use std::ffi::c_void;
use windows::core::HRESULT;
use windows::Win32::System::Console::AllocConsole;
use windows::Win32::Foundation::{BOOL, HMODULE};
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use windows::Win32::Graphics::Dxgi::IDXGISwapChain;

// Import ocular-rs
use ocular;

// Our function we are going to hook with.
fn hk_present(this: IDXGISwapChain, sync_interval: u32, flags: u32) -> HRESULT {
    println!("Present called!");

    // Call and return the result of the original method.
    ocular::get_present().expect("Uh oh. Present isn't hooked?!").call(this, sync_interval, flags)
}


fn main() {
    // Create our Present hook.
    ocular::hook_present(hk_present);
}

// Boilerplate DLL entry.
#[no_mangle]
extern "system" fn DllMain(_dll_module: HMODULE, call_reason: u32, _reserved: *mut c_void) -> BOOL {
    match call_reason {
        DLL_PROCESS_ATTACH => {
            let _thread = std::thread::Builder::new().name("ocular".to_string()).spawn(|| {
                main();
            });
        },
        DLL_PROCESS_DETACH => (),
        _ => (),
    }

    BOOL::from(true)
}

```

## Implemented Hooks
| SwapChain     | Device                | DeviceContext     |
| ---           | ---                   | ---               |
| Present       | CreateVertexShader    | OMSetRenderTargets|
| ResizeBuffers | CreatePixelShader     |                   |
| ResizeTarget  |                       |                   |

## Please Note
* This is not done, not even close to all the DX11 methods have been implemented, I'm adding them as I need them for projects. You are welcome to request/add others and make a pull request.
* The main priority of the library is simplicity of use, which can sometimes be to the code's detriment.
