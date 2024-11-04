////////////////////////////////////////////////////////////
// Created by Woeful Wolf (https://github.com/WoefulWolf) //
// Tether is a drag & drop DLL proxy                      //
////////////////////////////////////////////////////////////

use std::ffi::c_void;

use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
use windows::core::{GUID, HRESULT, PCSTR, IUnknown};
use windows::Win32::Foundation::HMODULE;

// dxgi
type CreateDXGIFactoryFn = unsafe extern "stdcall" fn(
    riid: *const GUID,
    ppfactory: *mut *mut c_void,
) -> HRESULT;

#[no_mangle]
unsafe extern "stdcall" fn tether_CreateDXGIFactory(
    riid: *const GUID,
    ppfactory: *mut *mut c_void,
) -> HRESULT {
    let dxgi = LoadLibraryA(PCSTR(b"C:\\Windows\\System32\\dxgi.dll\0".as_ptr())).unwrap();
    let createdxgifactory: CreateDXGIFactoryFn = std::mem::transmute(GetProcAddress(dxgi, PCSTR(b"CreateDXGIFactory\0".as_ptr())));

    createdxgifactory(riid, ppfactory)
}

type CreateDXGIFactory1Fn = unsafe extern "stdcall" fn(
    riid: *const GUID,
    ppfactory: *mut *mut c_void,
) -> HRESULT;

#[no_mangle]
unsafe extern "stdcall" fn tether_CreateDXGIFactory1(
    riid: *const GUID,
    ppfactory: *mut *mut c_void,
) -> HRESULT {
    let dxgi = LoadLibraryA(PCSTR(b"C:\\Windows\\System32\\dxgi.dll\0".as_ptr())).unwrap();
    let createdxgifactory1: CreateDXGIFactory1Fn = std::mem::transmute(GetProcAddress(dxgi, PCSTR(b"CreateDXGIFactory1\0".as_ptr())));

    createdxgifactory1(riid, ppfactory)
}

type CreateDXGIFactory2Fn = unsafe extern "stdcall" fn(
    flags: u32,
    riid: *const GUID,
    ppfactory: *mut *mut c_void,
) -> HRESULT;

#[no_mangle]
unsafe extern "stdcall" fn tether_CreateDXGIFactory2(
    flags: u32,
    riid: *const GUID,
    ppfactory: *mut *mut c_void,
) -> HRESULT {
    let dxgi = LoadLibraryA(PCSTR(b"C:\\Windows\\System32\\dxgi.dll\0".as_ptr())).unwrap();
    let createdxgifactory2: CreateDXGIFactory2Fn = std::mem::transmute(GetProcAddress(dxgi, PCSTR(b"CreateDXGIFactory2\0".as_ptr())));

    createdxgifactory2(flags, riid, ppfactory)
}

// dinput8
type DirectInput8CreateFn = unsafe extern "stdcall" fn(
    hinst: HMODULE,
    dwversion: u32,
    riidltf: *const GUID,
    ppvout: *mut *mut c_void,
    punkouter: IUnknown,
) -> HRESULT;

#[no_mangle]
unsafe extern "stdcall" fn tether_DirectInput8Create(
    hinst: HMODULE,
    dwversion: u32,
    riidltf: *const GUID,
    ppvout: *mut *mut c_void,
    punkouter: IUnknown,
) -> HRESULT {
    let dinput8 = LoadLibraryA(PCSTR(b"C:\\Windows\\System32\\dinput8.dll\0".as_ptr())).unwrap();
    let directinput8create: DirectInput8CreateFn = std::mem::transmute(GetProcAddress(dinput8, PCSTR(b"DirectInput8Create\0".as_ptr())));

    directinput8create(hinst, dwversion, riidltf, ppvout, punkouter)
}

// d3d11
type D3D11CreateDeviceFn = unsafe extern "stdcall" fn(
    adapter: *mut c_void,
    driver_type: u32,
    software: *mut c_void,
    flags: u32,
    feature_levels: *const u32,
    feature_levels_count: u32,
    sdk_version: u32,
    device: *mut *mut c_void,
    feature_level: *mut u32,
    immediate_context: *mut *mut c_void,
) -> HRESULT;

#[no_mangle]
unsafe extern "stdcall" fn tether_D3D11CreateDevice(
    adapter: *mut c_void,
    driver_type: u32,
    software: *mut c_void,
    flags: u32,
    feature_levels: *const u32,
    feature_levels_count: u32,
    sdk_version: u32,
    device: *mut *mut c_void,
    feature_level: *mut u32,
    immediate_context: *mut *mut c_void,
) -> HRESULT {
    let d3d11 = LoadLibraryA(PCSTR(b"C:\\Windows\\System32\\d3d11.dll\0".as_ptr())).unwrap();
    let d3d11createdevice: D3D11CreateDeviceFn = std::mem::transmute(GetProcAddress(d3d11, PCSTR(b"D3D11CreateDevice\0".as_ptr())));

    d3d11createdevice(adapter, driver_type, software, flags, feature_levels, feature_levels_count, sdk_version, device, feature_level, immediate_context)
}

type D3D11CreateDeviceAndSwapChainFn = unsafe extern "stdcall" fn(
    adapter: *mut c_void,
    driver_type: u32,
    software: *mut c_void,
    flags: u32,
    feature_levels: *const u32,
    feature_levels_count: u32,
    sdk_version: u32,
    swap_chain_desc: *const c_void,
    swap_chain: *mut *mut c_void,
    device: *mut *mut c_void,
    feature_level: *mut u32,
    immediate_context: *mut *mut c_void,
) -> HRESULT;

#[no_mangle]
unsafe extern "stdcall" fn tether_D3D11CreateDeviceAndSwapChain(
    adapter: *mut c_void,
    driver_type: u32,
    software: *mut c_void,
    flags: u32,
    feature_levels: *const u32,
    feature_levels_count: u32,
    sdk_version: u32,
    swap_chain_desc: *const c_void,
    swap_chain: *mut *mut c_void,
    device: *mut *mut c_void,
    feature_level: *mut u32,
    immediate_context: *mut *mut c_void,
) -> HRESULT {
    let d3d11 = LoadLibraryA(PCSTR(b"C:\\Windows\\System32\\d3d11.dll\0".as_ptr())).unwrap();
    let d3d11createdeviceandswapchain: D3D11CreateDeviceAndSwapChainFn = std::mem::transmute(GetProcAddress(d3d11, PCSTR(b"D3D11CreateDeviceAndSwapChain\0".as_ptr())));

    d3d11createdeviceandswapchain(adapter, driver_type, software, flags, feature_levels, feature_levels_count, sdk_version, swap_chain_desc, swap_chain, device, feature_level, immediate_context)
}