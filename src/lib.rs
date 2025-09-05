#![allow(static_mut_refs)] // TODO: get this shit outta here

use std::collections::VecDeque;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::{mpsc, Mutex};
use windows::core::HRESULT;
use windows::Win32::System::Console::AllocConsole;
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use windows::Win32::Foundation::{BOOL, HMODULE, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dxgi::{IDXGISwapChain, DXGI_SWAP_CHAIN_DESC};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;
use windows::Win32::Graphics::Direct3D11::{ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11DepthStencilView};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, GetClientRect, SetWindowLongPtrA,
    GWLP_WNDPROC, WM_MOUSEMOVE, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WNDPROC,
};

use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::Devices::HumanInterfaceDevice::{
    DirectInput8Create,
    IDirectInput8A, IDirectInputDevice8A,
    GUID_SysMouse, GUID_SysKeyboard, DIKEYBOARD_W, DIKEYBOARD_A, DIKEYBOARD_S, DIKEYBOARD_D, DIKEYBOARD_SPACE, DIKEYBOARD_ESCAPE, DIKEYBOARD_UP, DIKEYBOARD_DOWN, DIKEYBOARD_LEFT, DIKEYBOARD_RIGHT, DIKEYBOARD_RETURN,
};
use windows::core::ComInterface;
use windows::core::Interface;
use retour::GenericDetour;
use once_cell::sync::Lazy;

use directx_math::XMMatrix;

use tracing::*;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use egui_directx11::DirectX11Renderer;
use egui_win32::InputManager;

mod tether;
pub mod gamedata;
pub mod config;
pub mod pathlog;
pub mod pathdata;
pub mod rendering;
pub mod ui;
pub mod error;
pub mod events;

use pathlog::*;
use rendering::*;
use ui::*;
use events::*;
use config::*;

use ocular;
use pintar::Pintar;

#[derive(Debug, Clone, Copy, Default)]
pub struct ScreenDimensions {
    pub render_size: (u32, u32),
    pub window_size: (u32, u32),
}

// static TICKRATE : u32 = 60;

static SCREEN_DIMENSIONS: Lazy<Mutex<ScreenDimensions>> = Lazy::new(|| Mutex::new(ScreenDimensions::default()));

pub static PATHLOG: Lazy<Mutex<PathLog>> = Lazy::new(|| Mutex::new(PathLog::init()));
pub static CONFIG_STATE: Lazy<Mutex<ConfigState>> = Lazy::new(|| Mutex::new(ConfigState::init()));
pub static UISTATE: Lazy<Mutex<UIState>> = Lazy::new(|| Mutex::new(UIState::init()));
pub static EVENTS: Lazy<Mutex<VecDeque<CelEvent>>> = Lazy::new(|| Mutex::new(VecDeque::new()));
pub static RENDER_UPDATES: Lazy<Mutex<RenderUpdates>> = Lazy::new(|| Mutex::new(RenderUpdates::new()));

// struct InputState {
//     input_manager: Option<InputManager>,
//     old_wndproc: Option<WNDPROC>,
//     cursor_on_ui: bool,
//     ui_wants_keyboard: bool,
// }

static mut PINTAR : Option<Pintar> = None;
static mut EGUI_RENDERER : Option<DirectX11Renderer> = None;

static mut INPUT_MANAGER : Option<InputManager> = None;
static mut OLD_WNDPROC : Option<WNDPROC> = None;
static mut IS_POINTER_OVER_EGUI : bool = false;
static mut EGUI_WANTS_KEYBOARD_INPUT : bool = false;

type GetDeviceStatusFn = unsafe extern "system" fn(this: *mut c_void, param0: u32, param1: *mut c_void) -> HRESULT;

static MOUSE_GET_DEVICE_STATE_HOOK: Lazy<GenericDetour<GetDeviceStatusFn>> = Lazy::new(|| {
    unsafe {
        let mut di8: Option<IDirectInput8A> = None;
        let _res = DirectInput8Create(GetModuleHandleA(None).unwrap(), 0x0800, &IDirectInput8A::IID, std::mem::transmute(&mut di8), None);

        let mut di8_device_mouse: Option<IDirectInputDevice8A> = None;
        let _res = di8.clone().unwrap().CreateDevice(&GUID_SysMouse, &mut di8_device_mouse, None);

        let hook = GenericDetour::<GetDeviceStatusFn>::new(std::mem::transmute(di8_device_mouse.unwrap().vtable().GetDeviceState), hk_mouse_get_device_state).expect("Failed to hook GetDeviceState for mouse.");

        hook
    }
  });

static KEYBOARD_GET_DEVICE_STATE_HOOK: Lazy<GenericDetour<GetDeviceStatusFn>> = Lazy::new(|| {
    unsafe {
        let mut di8: Option<IDirectInput8A> = None;
        let _res = DirectInput8Create(GetModuleHandleA(None).unwrap(), 0x0800, &IDirectInput8A::IID, std::mem::transmute(&mut di8), None);

        let mut di8_device_keyboard: Option<IDirectInputDevice8A> = None;
        let _res = di8.unwrap().CreateDevice(&GUID_SysKeyboard, &mut di8_device_keyboard, None);

        let hook = GenericDetour::<GetDeviceStatusFn>::new(std::mem::transmute(di8_device_keyboard.unwrap().vtable().GetDeviceState), hk_keyboard_get_device_state).expect("Failed to hook GetDeviceState for keyboard.");

        hook
    }
  });

pub enum RX {
    Save { rx: mpsc::Receiver<Result<Option<PathBuf>, native_dialog::Error>> },
    Load { rx: mpsc::Receiver<Result<Option<PathBuf>, native_dialog::Error>> },
}

// pub struct DebugState {
//     frame_time: u64,
//     copy_time: u64,

//     frame_count: usize,
//     // calc_avg: bool,
//     last_frame: Instant,
//     last_player_pos: Vec3,
//     player_speeds: Vec<f32>,
//     average_player_speed: f32,
// }

unsafe fn init_globals(this: &IDXGISwapChain) {
    let mut sd: DXGI_SWAP_CHAIN_DESC = std::mem::zeroed();
    let _ = this.GetDesc(&mut sd);

    if let Ok(mut dims) = SCREEN_DIMENSIONS.lock() {
        let mut client_rect = RECT::default();
        if unsafe { GetClientRect(sd.OutputWindow, &mut client_rect).is_ok() } {
            dims.window_size.0 = (client_rect.right - client_rect.left) as u32;
            dims.window_size.1 = (client_rect.bottom - client_rect.top) as u32;
        }
    }

    if PINTAR.is_none() {
        let mut pintar = Pintar::init(&this, 0);
        pintar.add_line_vertex_group(RECORDING_GROUP.to_string());
        pintar.add_line_vertex_group(PATHS_GROUP.to_string());
        pintar.add_default_vertex_group(TRIGGERS_GROUP.to_string());
        pintar.add_default_vertex_group(TELEPORTS_GROUP.to_string());
        pintar.add_default_vertex_group(SHAPES_GROUP.to_string());
        PINTAR = Some(pintar);
    }

    if EGUI_RENDERER.is_none() {
        match DirectX11Renderer::init_from_swapchain(&this, egui::Context::default()) {
            Ok(renderer) => {
                EGUI_RENDERER = Some(renderer);
            },
            Err(e) => error!("{e}")
        }
    }

    if INPUT_MANAGER.is_none() {
        OLD_WNDPROC = Some(std::mem::transmute(SetWindowLongPtrA(sd.OutputWindow, GWLP_WNDPROC, new_wndproc as usize as _)));
        INPUT_MANAGER = Some(InputManager::new(sd.OutputWindow));
    }
}

extern "system" fn hk_present(this: IDXGISwapChain, sync_interval: u32, flags: u32) -> HRESULT {
    // let frame_start = Instant::now();

    unsafe {
        init_globals(&this);

        let mut pathlog = PATHLOG.lock().unwrap();

        if gamedata::get_is_loading() {
            pathlog.pause();
        } else {
            pathlog.unpause();
        }

        let pathlog_updates = pathlog.update(&gamedata::get_player_position(), &gamedata::get_player_rotation());

        drop(pathlog);

        RENDER_UPDATES.lock().unwrap().or(pathlog_updates);

        if let Some(pintar) = PINTAR.as_mut() {
            let view_proj = gamedata::get_view_matrix();

            pintar.set_default_view_proj(view_proj);

            let line_vertex_group: &mut pintar::vertex_group::VertexGroup<pintar::default_elements::LineVertex, pintar::default_elements::DefaultConstants> = pintar.get_vertex_group_as(RECORDING_GROUP.to_string()).unwrap();
            line_vertex_group.constants.view_proj = XMMatrix::from(&view_proj);

            let line_vertex_group: &mut pintar::vertex_group::VertexGroup<pintar::default_elements::LineVertex, pintar::default_elements::DefaultConstants> = pintar.get_vertex_group_as(PATHS_GROUP.to_string()).unwrap();
            line_vertex_group.constants.view_proj = XMMatrix::from(&view_proj);

            let triggers_vertex_group: &mut pintar::vertex_group::VertexGroup<pintar::default_elements::DefaultVertex, pintar::default_elements::DefaultConstants> = pintar.get_vertex_group_as(TRIGGERS_GROUP.to_string()).unwrap();
            triggers_vertex_group.constants.view_proj = XMMatrix::from(&view_proj);

            let teleports_vertex_group: &mut pintar::vertex_group::VertexGroup<pintar::default_elements::DefaultVertex, pintar::default_elements::DefaultConstants> = pintar.get_vertex_group_as(TELEPORTS_GROUP.to_string()).unwrap();
            teleports_vertex_group.constants.view_proj = XMMatrix::from(&view_proj);

            let shapes_vertex_group: &mut pintar::vertex_group::VertexGroup<pintar::default_elements::DefaultVertex, pintar::default_elements::DefaultConstants> = pintar.get_vertex_group_as(SHAPES_GROUP.to_string()).unwrap();
            shapes_vertex_group.constants.view_proj = XMMatrix::from(&view_proj);

            pintar.clear_vertex_group(RECORDING_GROUP.to_string());
            rendering::render_path(pintar, RECORDING_GROUP.to_string(), &PATHLOG.lock().unwrap().recording_path, [1.0, 1.0, 1.0, 0.8], 0.02);

            pintar.clear_vertex_group(SHAPES_GROUP.to_string());
            rendering::render_custom_shapes(pintar);

            pintar.clear_vertex_group(TELEPORTS_GROUP.to_string());
            rendering::render_teleports(pintar);

            rendering::render_all_paths(pintar);

            pintar.clear_vertex_group(TRIGGERS_GROUP.to_string());
            rendering::render_triggers(pintar);

            pintar.render();
        }

        if let Some(dx_renderer) = EGUI_RENDERER.as_mut() {
            let input = match INPUT_MANAGER {
                Some(ref mut input_manager) => input_manager.collect_input().unwrap(),
                None => egui::RawInput::default(),
            };

            ui::check_input(&input);

            // i have to pass in some reference idk
            let mut nothing = 0;

            dx_renderer
                .paint(&this, &mut nothing, input.clone(), |ctx, _| {
                    // this changes the resolution its rendering but not the scale on screen for some fucking reason
                    // ctx.set_pixels_per_point(CONFIG_STATE.lock().unwrap().zoom);

                    // my best attempt at getting a fucking window position but ofc it's private
                    // let state = ctx.memory(|mem| mem.areas().get(id).copied());

                    IS_POINTER_OVER_EGUI = ctx.is_pointer_over_area();
                    EGUI_WANTS_KEYBOARD_INPUT = ctx.wants_keyboard_input();

                    egui::Window::new("Celestial")
                        .default_size(egui::vec2(300f32, 300f32))
                        .vscroll(true)
                        .resizable(true)
                        .min_size([300.0, 300.0])
                        .frame(egui::Frame::window(&ctx.style()).inner_margin(7.0))
                        .show(ctx, |ui| {
                            ui::draw_ui(ui);
                        });

                    egui::Window::new("Timer")
                        .resizable(false)
                        .title_bar(false)
                        .frame(egui::Frame::window(&ctx.style()).inner_margin(7.0))
                        .show(ctx, |ui| {
                            ui::draw_timer(ui);
                        });

                    #[cfg(debug_assertions)] {
                    egui::Window::new("Debug")
                        .resizable(true)
                        .frame(egui::Frame::window(&ctx.style()).inner_margin(7.0))
                        .show(ctx, |ui| {
                            ui::draw_debug(ui);
                        });
                    }
                    })
                .expect("successful render");

            events::process_events();
        }
    }

    // Call and return the result of the original method.
    ocular::get_present().expect("Uh oh. Present isn't hooked?!").call(this, sync_interval, flags)
}

extern "system" fn hk_resize_buffers(
    this: IDXGISwapChain,
    buffer_count: u32,
    width: u32,
    height: u32,
    new_format: DXGI_FORMAT,
    swap_chain_flags: u32,
) -> HRESULT {
    info!("ResizeBuffers called");

    if let Ok(mut dims) = SCREEN_DIMENSIONS.lock() {
        dims.render_size = (width, height);
    }

    let result;
    unsafe {

        if let Some(pintar) = PINTAR.as_mut() {
            pintar.drop_back_buffer_data();
        }

        if let Some(egui_renderer) = EGUI_RENDERER.as_mut() {
            result = egui_renderer.resize_buffers(&this, || {
                ocular::get_resize_buffers().unwrap().call(this.clone(), buffer_count, width, height, new_format, swap_chain_flags).ok()
            }).unwrap()
        } else {
            result = ocular::get_resize_buffers().unwrap().call(this.clone(), buffer_count, width, height, new_format, swap_chain_flags).ok();
        }

        if let Some(pintar) = PINTAR.as_mut() {
            pintar.update_back_buffer_data(&this);
        }
    }

    return result.into();
}

extern "system" fn hk_om_set_render_targets(
    context: ID3D11DeviceContext,
    num_views: u32,
    render_target_views: *const Option<ID3D11RenderTargetView>,
    depth_stencil_view: Option<ID3D11DepthStencilView>
) {
    unsafe {
        if let Some(pintar) = PINTAR.as_mut() {
            pintar.find_view_resources((*render_target_views).clone(), depth_stencil_view.clone());
        }
    }
    ocular::get_om_set_render_targets().expect("Uh oh. OMSetRenderTargets isn't hooked?!").call(context, num_views, render_target_views, depth_stencil_view)
}

unsafe extern "system" fn new_wndproc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam_original: usize,
    lparam_original: isize
) -> LRESULT {
    let wparam_winrs = wparam_original;
    let mut lparam_winrs = lparam_original;

    match msg {

        // thanks to Vluurie for this!
        WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP => {
            if let Ok(dims) = SCREEN_DIMENSIONS.lock() {
                if dims.window_size.0 > 0
                    && dims.window_size.1 > 0
                    && dims.window_size != dims.render_size
                {
                    let scale_x = dims.render_size.0 as f32 / dims.window_size.0 as f32;
                    let scale_y = dims.render_size.1 as f32 / dims.window_size.1 as f32;

                    let x = (lparam_original & 0xFFFF) as i16 as f32;
                    let y = (lparam_original >> 16) as i16 as f32;

                    let scaled_x = (x * scale_x).round() as isize;
                    let scaled_y = (y * scale_y).round() as isize;

                    lparam_winrs = (scaled_y << 16) | (scaled_x & 0xFFFF);
                }
            }
        }
        _ => {}
    }

    unsafe {
        if let Some(input_manager) = INPUT_MANAGER.as_mut() {
            input_manager.process(msg, wparam_winrs, lparam_winrs);
        }
    }

    CallWindowProcW(OLD_WNDPROC.unwrap(), hwnd, msg, WPARAM(wparam_winrs), LPARAM(lparam_winrs))
}

extern "system" fn hk_mouse_get_device_state(this: *mut c_void, param0: u32, param1: *mut c_void) -> HRESULT {
    unsafe {
        if IS_POINTER_OVER_EGUI {
            let _res = MOUSE_GET_DEVICE_STATE_HOOK.call(this, param0, param1);
            std::ptr::write_bytes(param1, 0, param0 as usize);
            return HRESULT(1)
        }

        MOUSE_GET_DEVICE_STATE_HOOK.call(this, param0, param1)
    }
}

extern "system" fn hk_keyboard_get_device_state(this: *mut c_void, param0: u32, param1: *mut c_void) -> HRESULT {
    unsafe {
        if EGUI_WANTS_KEYBOARD_INPUT {
            let _res = KEYBOARD_GET_DEVICE_STATE_HOOK.call(this, param0, param1);
            std::ptr::write_bytes(param1, 0, param0 as usize);
            return HRESULT(1)
        }

        let _res = KEYBOARD_GET_DEVICE_STATE_HOOK.call(this, param0, param1);

        let keys_pressed: [u8; 256] = std::ptr::read((param1) as *const _);

        std::ptr::write_bytes(param1, 0, param0 as usize);

        std::ptr::write_bytes((param1 as usize + (DIKEYBOARD_SPACE & 0xFF) as usize) as *mut c_void, keys_pressed[(DIKEYBOARD_SPACE & 0xFF) as usize], 1);
        std::ptr::write_bytes((param1 as usize + (DIKEYBOARD_W & 0xFF) as usize) as *mut c_void, keys_pressed[(DIKEYBOARD_W & 0xFF) as usize], 1);
        std::ptr::write_bytes((param1 as usize + (DIKEYBOARD_A & 0xFF) as usize) as *mut c_void, keys_pressed[(DIKEYBOARD_A & 0xFF) as usize], 1);
        std::ptr::write_bytes((param1 as usize + (DIKEYBOARD_S & 0xFF) as usize) as *mut c_void, keys_pressed[(DIKEYBOARD_S & 0xFF) as usize], 1);
        std::ptr::write_bytes((param1 as usize + (DIKEYBOARD_D & 0xFF) as usize) as *mut c_void, keys_pressed[(DIKEYBOARD_D & 0xFF) as usize], 1);

        std::ptr::write_bytes((param1 as usize + (DIKEYBOARD_UP & 0xFF) as usize) as *mut c_void, keys_pressed[(DIKEYBOARD_UP & 0xFF) as usize], 1);
        std::ptr::write_bytes((param1 as usize + (DIKEYBOARD_DOWN & 0xFF) as usize) as *mut c_void, keys_pressed[(DIKEYBOARD_DOWN & 0xFF) as usize], 1);
        std::ptr::write_bytes((param1 as usize + (DIKEYBOARD_LEFT & 0xFF) as usize) as *mut c_void, keys_pressed[(DIKEYBOARD_LEFT & 0xFF) as usize], 1);
        std::ptr::write_bytes((param1 as usize + (DIKEYBOARD_RIGHT & 0xFF) as usize) as *mut c_void, keys_pressed[(DIKEYBOARD_RIGHT & 0xFF) as usize], 1);
        std::ptr::write_bytes((param1 as usize + (DIKEYBOARD_RETURN & 0xFF) as usize) as *mut c_void, keys_pressed[(DIKEYBOARD_RETURN & 0xFF) as usize], 1);
        std::ptr::write_bytes((param1 as usize + (DIKEYBOARD_ESCAPE & 0xFF) as usize) as *mut c_void, keys_pressed[(DIKEYBOARD_ESCAPE & 0xFF) as usize], 1);

        return HRESULT(1)

        // KEYBOARD_GET_DEVICE_STATE_HOOK.call(this, param0, param1)
    }
}

fn delete_old_logs(logs_path: PathBuf, log_prefix: impl Into<String>, rotation_count: usize) {
    let log_prefix: String = log_prefix.into();
    // Get all files in the logs directory that start with the log prefix
    let mut log_files = std::fs::read_dir(logs_path).expect("Failed to read logs directory")
        .filter_map(|entry| {
            let entry = entry.expect("Failed to read log file entry");
            let path = entry.path();
            let file_name = path.file_name().unwrap().to_str().unwrap().to_string();
            if file_name.starts_with(&log_prefix) {
                Some(path)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if log_files.len() <= rotation_count {
        return;
    }

    // Sort the log files
    log_files.sort_by(|a, b| {
        let a = a.file_name().unwrap().to_str().unwrap();
        let b = b.file_name().unwrap().to_str().unwrap();
        a.cmp(b)
    });

    // Delete the oldest log files
    let log_files_to_delete = log_files.iter().take(log_files.len() - rotation_count);
    for log_file in log_files_to_delete {
        std::fs::remove_file(log_file).expect("Failed to delete old log file");
    }
}

fn log_setup() {
    std::panic::set_hook(Box::new(|panic_info| {
        error!("{}", panic_info);
    }));

    #[cfg(debug_assertions)] {
        unsafe { let _ = AllocConsole(); };
    }
    let cwd = std::env::current_dir().expect("Failed to get current directory");
    let logs_path = cwd.join("logs");
    let log_file_name_prefix = "celestial.log";
    let _ = std::fs::create_dir_all(logs_path.clone());

    delete_old_logs(logs_path.clone(), log_file_name_prefix, 5);

    let file_appender = tracing_appender::rolling::daily(logs_path, log_file_name_prefix);

    let file_log = tracing_subscriber::fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_file(false);

    let subscriber = tracing_subscriber::Registry::default()
        .with(file_log)
        .with(EnvFilter::from_default_env().add_directive(tracing_subscriber::filter::LevelFilter::INFO.into()));

    #[cfg(not(debug_assertions))] {
        tracing::subscriber::set_global_default(subscriber).expect("Unable to set global subscriber");
    }

    #[cfg(debug_assertions)] {
        let stdout_log = tracing_subscriber::fmt::layer().compact()
            .with_ansi(false)
            .with_file(false);

        let subscriber = subscriber.with(stdout_log);
        tracing::subscriber::set_global_default(subscriber).expect("Unable to set global subscriber");
    }
}

fn main() {
    log_setup();

    unsafe {
        // let mut debug = DebugState{
        //     frame_time: 0,
        //     copy_time: 0,
        //     frame_count: 0,
        //     // calc_avg: false,
        //     last_frame: Instant::now(),
        //     last_player_pos: Vec3::ZERO,
        //     player_speeds: Vec::with_capacity(60),
        //     average_player_speed: 0.,
        // };
        // DEBUG_STATE.as_mut().unwrap().player_speeds.resize(60, 0.);
        // debug.player_speeds.resize(60, 0.);

        // DEBUG_STATE = Some(debug);

        MOUSE_GET_DEVICE_STATE_HOOK.enable().unwrap();
        KEYBOARD_GET_DEVICE_STATE_HOOK.enable().unwrap();
    }

    ocular::hook_present(hk_present);
    ocular::hook_resize_buffers(hk_resize_buffers);
    ocular::hook_om_set_render_targets(hk_om_set_render_targets);
}

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

// changing page protection flags for code injection
// maybe ill need this for something
// let mut old = windows::Win32::System::Memory::PAGE_PROTECTION_FLAGS(0);
// let wo = windows::Win32::System::Memory::VirtualProtect(addr as *const , 8, windows::Win32::System::Memory::PAGEPROTECTION_FLAGS(0x4), &mut old as *mut );