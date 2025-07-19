#![allow(static_mut_refs)]

use std::ffi::c_void;
use std::path::PathBuf;
use std::time::Instant;
use std::collections::VecDeque;
use glam::Vec3;
use windows::core::HRESULT;
use windows::Win32::System::Console::AllocConsole;
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use windows::Win32::Foundation::{BOOL, HMODULE, LRESULT, WPARAM, LPARAM};
use windows::Win32::Graphics::Dxgi::{IDXGISwapChain, DXGI_SWAP_CHAIN_DESC};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;
use windows::Win32::Graphics::Direct3D11::{ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11DepthStencilView};
use windows::Win32::UI::WindowsAndMessaging::{SetWindowLongPtrA, CallWindowProcW, WNDPROC, GWLP_WNDPROC};

use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::Devices::HumanInterfaceDevice::{DirectInput8Create, IDirectInput8A, IDirectInputDevice8A, GUID_SysMouse, GUID_SysKeyboard};
use windows::core::ComInterface;
use windows::core::Interface;
use retour::GenericDetour;
use once_cell::sync::Lazy;

use tracing::*;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use egui::RichText;
use egui_directx11::DirectX11Renderer;
use egui_win32::InputManager;
use uuid::Uuid;

mod tether;
pub mod gamedata;
pub mod config;
pub mod pathlog;
pub mod pathdata;
pub mod ui;
pub mod error;

use config::*;
use pathlog::*;
use pathdata::*;
use ui::*;

use ocular;
use pintar::Pintar;

// static TICKRATE : u32 = 60;

static mut GLOBAL_STATE : Option<GlobalState> = None;
static mut DEBUG_STATE : Option<DebugState> = None;

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

// enum GlobalEvent {
//     CollectionUpdate,
//     TriggersUpdate,
//     TeleportsUpdate,
//     ShapesUpdate,
// }

struct RenderUpdates {
    paths: bool,
    triggers: bool,
    teleports: bool,
    shapes: bool,
}

struct GlobalState {
    pathlog: PathLog,
    config: ConfigState,
    egui: UIState,
    updates: RenderUpdates,
    // events: VecDeque<GlobalEvent>,
}

impl GlobalState {
    pub fn init() -> GlobalState {
        GlobalState {
            pathlog: PathLog::init(),
            config: ConfigState::init(),
            egui: UIState::init(),
            updates: RenderUpdates { paths: false, triggers: false, teleports: false, shapes: false }
            // events: VecDeque::new(),
        }
    }
}

pub struct DebugState {
    frame_time: u64,
    copy_time: u64,

    frame_count: usize,
    // calc_avg: bool,
    last_frame: Instant,
    last_player_pos: Vec3,
    player_speeds: Vec<f32>,
    average_player_speed: f32,
}

unsafe fn init_globals(this: &IDXGISwapChain) {
    let mut sd: DXGI_SWAP_CHAIN_DESC = std::mem::zeroed();
    let _ = this.GetDesc(&mut sd);

    if PINTAR.is_none() {
        PINTAR = Some(Pintar::new(&this, 0));
    }

    if EGUI_RENDERER.is_none() {
        EGUI_RENDERER = Some(DirectX11Renderer::init_from_swapchain(&this, egui::Context::default()).unwrap()); // TODO: check for errors maybe?
    }

    if INPUT_MANAGER.is_none() {
        OLD_WNDPROC = Some(std::mem::transmute(SetWindowLongPtrA(sd.OutputWindow, GWLP_WNDPROC, new_wndproc as usize as _)));
        INPUT_MANAGER = Some(InputManager::new(sd.OutputWindow));
    }
}

extern "system" fn hk_present(this: IDXGISwapChain, sync_interval: u32, flags: u32) -> HRESULT {
    let frame_start = Instant::now();

    unsafe {
        init_globals(&this);

        let debug = DEBUG_STATE.as_mut().unwrap();

        let elapsed = debug.last_frame.elapsed().as_secs_f32();

        let speed = (Vec3::from_array(gamedata::get_player_position()) - debug.last_player_pos).length() / elapsed;
        debug.player_speeds[debug.frame_count % 60] = speed;
        // debug.player_speeds.push(speed);
        // if debug.calc_avg {
        if debug.frame_count % 10 == 0 {
            debug.average_player_speed = debug.player_speeds.iter().sum::<f32>() / debug.player_speeds.len() as f32;
            // debug.player_speeds.clear();
            // debug.calc_avg = false;
        }

        debug.last_player_pos = gamedata::get_player_position().into();
        debug.last_frame = Instant::now();
        debug.frame_count += 1;

        if let Some(pintar) = PINTAR.as_mut() {
            DEBUG_STATE.as_mut().unwrap().copy_time = 0;

            let state = &mut GLOBAL_STATE.as_mut().unwrap().egui;
            let pathlog = &mut GLOBAL_STATE.as_mut().unwrap().pathlog;
            let config = &GLOBAL_STATE.as_mut().unwrap().config;
            let updates = &mut GLOBAL_STATE.as_mut().unwrap().updates;
            // let events = &mut GLOBAL_STATE.as_mut().unwrap().events;

            if gamedata::get_is_loading() {
                pathlog.pause();
            } else {
                pathlog.unpause();
            }

            pathlog.update(&gamedata::get_player_position(), &gamedata::get_player_rotation(), updates);

            pintar.set_default_view_proj(gamedata::get_view_matrix());

            render_path(pintar, &pathlog.recording_path, [1.0, 1.0, 1.0, 0.8], 0.02);

            render_custom_shapes(pintar, state);

            // for shape in &state.custom_shapes {
            //     if shape.1 { continue; }
            //     match shape.0.shape_type {
            //         ShapeType::Box => {
            //             pintar.add_default_mesh(pintar::primitives::cube::new(shape.0.color.to_rgba_premultiplied())
            //                 .scale(shape.0.size)
            //                 .rotate(shape.0.rotation)
            //                 .translate(shape.0.position));
            //         }
            //         ShapeType::Sphere => {
            //             let mut size = shape.0.size;
            //             size[1] = size[0];
            //             size[2] = size[0];
            //             pintar.add_default_mesh(pintar::primitives::sphere::new(shape.0.color.to_rgba_premultiplied())
            //                 .scale(size)
            //                 .translate(shape.0.position));
            //         }
            //         ShapeType::Cylinder => {
            //             let mut size = shape.0.size;
            //             size[2] = size[0];
            //             pintar.add_default_mesh(pintar::primitives::cylinder::new(shape.0.color.to_rgba_premultiplied())
            //                 .scale(size)
            //                 .translate(shape.0.position));
            //         }
            //     }
            // }

            render_teleports(pintar, state, config);

            // if let Some(teleport) = &state.teleports[0] {
            //     let pos = teleport.location;
            //     let mut color = config.trigger_color[0];
            //     // color[3] = 0.5;
            //     // pos[1] += 1.0;
            //     pintar.add_default_mesh(pintar::primitives::cylinder::new(color).scale([0.6, 0.05, 0.6]).translate(pos));
            //     color[3] *= 0.25;
            //     pintar.add_default_mesh(pintar::primitives::cylinder::new(color).scale([0.5, 0.051, 0.5]).translate(pos));
            // }

            // if let Some(teleport) = &state.teleports[1] {
            //     let pos = teleport.location;
            //     let mut color = config.trigger_color[1];
            //     // color[3] = 0.5;
            //     // pos[1] += 1.0;
            //     pintar.add_default_mesh(pintar::primitives::cylinder::new(color).scale([0.6, 0.05, 0.6]).translate(pos));
            //     color[3] *= 0.25;
            //     pintar.add_default_mesh(pintar::primitives::cylinder::new(color).scale([0.5, 0.051, 0.5]).translate(pos));
            // }


            // TODO: problem: need more control over clearing vertex groups
            if updates.paths {
                render_all_paths(pintar, state, config, pathlog);
                updates.paths = false;
            }

            // let mut visible_collection = PathCollection::new("Visible".to_string());
            // let mut selected : Vec<Uuid> = Vec::new();

            // for collection in &pathlog.path_collections {
            //     let mut visible = true;
            //     for v in state.solo_collections.values() {
            //         if *v {visible = false;}
            //     }

            //     if !visible { continue; }

            //     for path in collection.paths() {
            //         if visible_collection.paths().contains(path) { continue; }

            //         visible_collection.add(path.clone(), None);

            //         if state.selected_paths.get(&collection.id()).unwrap().contains(&path.id()) {
            //             selected.push(path.id());
            //         }
            //     }
            // }

            // let visible_paths = visible_collection.paths();
            // for i in 0..visible_paths.len() {
            //     let path = &visible_paths[i];

            //     let mut visible = true;
            //     for v in state.solo_paths.values() {
            //         if *v {visible = false;}
            //     }
            //     if state.solo_paths.get(&path.id()) == Some(&true) { visible = true; }
            //     if state.mute_paths.get(&path.id()) == Some(&true) { visible = false; }
            //     if !visible { continue; }

            //     let fast = config.fast_color;
            //     let slow = config.slow_color;
            //     let lerp = |a: f32, b: f32, t: f32| -> f32 { a * (1.0-t) + b * t };
            //     let mut color: [f32; 4];
            //     let thick: f32;

            //     if i == 0 {
            //         color = config.gold_color;
            //         thick = 0.04;
            //     }
            //     else if visible_paths.len() == 2 {
            //         color = config.slow_color;
            //         thick = 0.02;
            //     }
            //     else {
            //         let p = (i - 1) as f32 / (visible_paths.len() - 2) as f32;

            //         color = [
            //             lerp(fast[0], slow[0], p),
            //             lerp(fast[1], slow[1], p),
            //             lerp(fast[2], slow[2], p),
            //             lerp(fast[3], slow[3], p),
            //         ];
            //         thick = 0.02;
            //     }

            //     if selected.contains(&path.id()) {
            //         color = config.select_color;
            //     }

            //     render_path(pintar, &path, color, thick);
            // }

            // for path in pathlog.direct_paths.paths() {
            //     // if !state.mute_paths.contains_key(&path.id()) { state.mute_paths.insert(path.id(), false); }
            //     // if !state.solo_paths.contains_key(&path.id()) { state.solo_paths.insert(path.id(), false); }

            //     let mut visible = true;
            //     for v in state.solo_paths.values() {
            //         if *v {visible = false;}
            //     }
            //     if state.solo_paths.get(&path.id()) == Some(&true) { visible = true; }
            //     if state.mute_paths.get(&path.id()) == Some(&true) { visible = false; }
            //     if !visible { continue; }
            //     render_path(&path, pintar, [1.0, 1.0, 1.0, 1.0], 0.02);
            // }

            render_triggers(pintar, config, pathlog);

            // for collider in &pathlog.checkpoint_triggers {
            //     pintar.add_default_mesh(pintar::primitives::cube::new(config.checkpoint_color).scale(collider.size).rotate(collider.rotation()).translate(collider.position));
            // }

            // for i in 0..2 {
            //     if let Some(collider) = pathlog.main_triggers[i] {
            //         pintar.add_default_mesh(pintar::primitives::cube::new(config.trigger_color[i]).scale(collider.size).rotate(collider.rotation()).translate(collider.position));
            //     }
            // }

            pintar.render();
            pintar.clear_vertex_groups();
        }

        if let Some(dx_renderer) = EGUI_RENDERER.as_mut() {
            let input = match INPUT_MANAGER {
                Some(ref mut input_manager) => input_manager.collect_input().unwrap(),
                None => egui::RawInput::default(),
            };

            let mut global_state = GLOBAL_STATE.as_mut().unwrap();

            ui::check_input(&input, &mut global_state.egui, &mut global_state.config);

            dx_renderer
                .paint(&this, &mut global_state, input.clone(), |ctx, state| {
                    egui::Window::new("Celestial")
                        .default_size(egui::vec2(300f32, 300f32))
                        .vscroll(true)
                        .resizable(true)
                        .min_size([300.0, 300.0])
                        .frame(egui::Frame::window(&ctx.style()).inner_margin(7.0))
                        .show(ctx, |ui| {
                            IS_POINTER_OVER_EGUI = ctx.is_pointer_over_area();
                            EGUI_WANTS_KEYBOARD_INPUT = ctx.wants_keyboard_input();
                            ui::draw_ui(ui, &mut state.egui, &mut state.config, &mut state.pathlog);
                        });

                    egui::Window::new("Timer")
                        .resizable(false)
                        .title_bar(false)
                        .frame(egui::Frame::window(&ctx.style()).inner_margin(7.0))
                        .show(ctx, |ui| {
                            ui::draw_timer(ui, &mut state.config, &mut state.pathlog);
                        });

                    #[cfg(debug_assertions)] {
                    egui::Window::new("Debug")
                        .resizable(true)
                        .frame(egui::Frame::window(&ctx.style()).inner_margin(7.0))
                        .show(ctx, |ui| {
                            draw_debug(ui);
                        });
                    }
                    })
                .expect("successful render");

            global_state.egui.process_events(&mut global_state.pathlog);
        }

        DEBUG_STATE.as_mut().unwrap().frame_time = frame_start.elapsed().as_micros() as u64;
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

fn render_path(pintar: &mut Pintar, path: &Path, color: [f32; 4], thickness: f32) {
    for segment in path.segments() {
        if segment.len() < 2 { continue; }
        pintar.add_line(segment, color, thickness);
    }
}

fn render_all_paths(pintar: &mut Pintar, state: &UIState, config: &ConfigState, pathlog: &PathLog) {
    let mut visible_collection = PathCollection::new("Visible".to_string());
    let mut selected : Vec<Uuid> = Vec::new();

    for collection in &pathlog.path_collections {
        let mut visible = true;
        for v in state.solo_collections.values() {
            if *v {visible = false;}
        }

        if !visible { continue; }

        for path in collection.paths() {
            if visible_collection.paths().contains(path) { continue; }

            visible_collection.add(path.clone(), None);

            if state.selected_paths.get(&collection.id()).unwrap().contains(&path.id()) {
                selected.push(path.id());
            }
        }
    }

    let visible_paths = visible_collection.paths();
    for i in 0..visible_paths.len() {
        let path = &visible_paths[i];

        let mut visible = true;
        for v in state.solo_paths.values() {
            if *v {visible = false;}
        }
        if state.solo_paths.get(&path.id()) == Some(&true) { visible = true; }
        if state.mute_paths.get(&path.id()) == Some(&true) { visible = false; }
        if !visible { continue; }

        let fast = config.fast_color;
        let slow = config.slow_color;
        let lerp = |a: f32, b: f32, t: f32| -> f32 { a * (1.0-t) + b * t };
        let mut color: [f32; 4];
        let thick: f32;

        if i == 0 {
            color = config.gold_color;
            thick = 0.04;
        }
        else if visible_paths.len() == 2 {
            color = config.slow_color;
            thick = 0.02;
        }
        else {
            let p = (i - 1) as f32 / (visible_paths.len() - 2) as f32;

            color = [
                lerp(fast[0], slow[0], p),
                lerp(fast[1], slow[1], p),
                lerp(fast[2], slow[2], p),
                lerp(fast[3], slow[3], p),
            ];
            thick = 0.02;
        }

        if selected.contains(&path.id()) {
            color = config.select_color;
        }

        render_path(pintar, &path, color, thick);
    }
}

fn render_triggers(pintar: &mut Pintar, config: &ConfigState, pathlog: &PathLog) {
    for collider in &pathlog.checkpoint_triggers {
        pintar.add_default_mesh(pintar::primitives::cube::new(config.checkpoint_color).scale(collider.size).rotate(collider.rotation()).translate(collider.position));
    }

    for i in 0..2 {
        if let Some(collider) = pathlog.main_triggers[i] {
            pintar.add_default_mesh(pintar::primitives::cube::new(config.trigger_color[i]).scale(collider.size).rotate(collider.rotation()).translate(collider.position));
        }
    }
}

fn render_teleports(pintar: &mut Pintar, state: &UIState, config: &ConfigState) {
    if let Some(teleport) = &state.teleports[0] {
        let pos = teleport.location;
        let mut color = config.trigger_color[0];
        // color[3] = 0.5;
        // pos[1] += 1.0;
        pintar.add_default_mesh(pintar::primitives::cylinder::new(color).scale([0.6, 0.05, 0.6]).translate(pos));
        color[3] *= 0.25;
        pintar.add_default_mesh(pintar::primitives::cylinder::new(color).scale([0.5, 0.051, 0.5]).translate(pos));
    }

    if let Some(teleport) = &state.teleports[1] {
        let pos = teleport.location;
        let mut color = config.trigger_color[1];
        // color[3] = 0.5;
        // pos[1] += 1.0;
        pintar.add_default_mesh(pintar::primitives::cylinder::new(color).scale([0.6, 0.05, 0.6]).translate(pos));
        color[3] *= 0.25;
        pintar.add_default_mesh(pintar::primitives::cylinder::new(color).scale([0.5, 0.051, 0.5]).translate(pos));
    }
}

fn render_custom_shapes(pintar: &mut Pintar, state: &UIState) {
    for shape in &state.custom_shapes {
        if shape.1 { continue; }
        match shape.0.shape_type {
            ShapeType::Box => {
                pintar.add_default_mesh(pintar::primitives::cube::new(shape.0.color.to_rgba_premultiplied())
                    .scale(shape.0.size)
                    .rotate(shape.0.rotation)
                    .translate(shape.0.position));
            }
            ShapeType::Sphere => {
                let mut size = shape.0.size;
                size[1] = size[0];
                size[2] = size[0];
                pintar.add_default_mesh(pintar::primitives::sphere::new(shape.0.color.to_rgba_premultiplied())
                    .scale(size)
                    .translate(shape.0.position));
            }
            ShapeType::Cylinder => {
                let mut size = shape.0.size;
                size[2] = size[0];
                pintar.add_default_mesh(pintar::primitives::cylinder::new(shape.0.color.to_rgba_premultiplied())
                    .scale(size)
                    .translate(shape.0.position));
            }
        }
    }
}

fn draw_debug(ui: &mut egui::Ui) {
    unsafe {
        let _debug = DEBUG_STATE.as_ref().unwrap();

        ui.add(egui::Label::new(
            RichText::new(format!("{:?}", gamedata::get_is_loading()))
        ).selectable(false));
    }
}

unsafe extern "system" fn new_wndproc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: usize,
    lparam: isize
) -> LRESULT {
    unsafe {
        if let Some(input_manager) = INPUT_MANAGER.as_mut() {
            input_manager.process(msg, wparam, lparam);
        }
    }

    CallWindowProcW(OLD_WNDPROC.unwrap(), hwnd, msg, WPARAM(wparam), LPARAM(lparam))
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

        KEYBOARD_GET_DEVICE_STATE_HOOK.call(this, param0, param1)
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
        GLOBAL_STATE = Some(GlobalState::init());
        let mut debug = DebugState{
            frame_time: 0,
            copy_time: 0,
            frame_count: 0,
            // calc_avg: false,
            last_frame: Instant::now(),
            last_player_pos: Vec3::ZERO,
            player_speeds: Vec::with_capacity(60),
            average_player_speed: 0.,
        };
        // DEBUG_STATE.as_mut().unwrap().player_speeds.resize(60, 0.);
        debug.player_speeds.resize(60, 0.);

        DEBUG_STATE = Some(debug);

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