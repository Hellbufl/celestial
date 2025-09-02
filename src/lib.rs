#![allow(static_mut_refs)] // TODO: get this shit outta here

use std::collections::VecDeque;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::{mpsc, Mutex};
use std::thread;
use native_dialog::FileDialog;
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

#[derive(Debug, Clone, Copy, Default)]
pub struct ScreenDimensions {
    pub render_size: (u32, u32),
    pub window_size: (u32, u32),
}

// static TICKRATE : u32 = 60;
static RECORDING_GROUP : &str = "recording";
static PATHS_GROUP : &str = "paths";
static TRIGGERS_GROUP : &str = "triggers";
static TELEPORTS_GROUP : &str = "teleports";
static SHAPES_GROUP : &str = "custom_shapes";

static SCREEN_DIMENSIONS: Lazy<Mutex<ScreenDimensions>> = Lazy::new(|| Mutex::new(ScreenDimensions::default()));

pub static PATHLOG: Lazy<Mutex<PathLog>> = Lazy::new(|| Mutex::new(PathLog::init()));
pub static CONFIG_STATE: Lazy<Mutex<ConfigState>> = Lazy::new(|| Mutex::new(ConfigState::init()));
pub static UISTATE: Lazy<Mutex<UIState>> = Lazy::new(|| Mutex::new(UIState::init()));
pub static EVENTS: Lazy<Mutex<VecDeque<UIEvent>>> = Lazy::new(|| Mutex::new(VecDeque::new()));
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

#[derive(Clone, Copy)]
pub struct RenderUpdates {
    pub paths: bool,
    pub triggers: bool,
    pub teleports: bool,
    pub shapes: bool,
}

impl RenderUpdates {
    pub fn new() -> Self {
        RenderUpdates { paths: false, triggers: false, teleports: false, shapes: false }
    }

    pub fn paths() -> Self {
        RenderUpdates { paths: true, triggers: false, teleports: false, shapes: false }
    }

    pub fn triggers() -> Self {
        RenderUpdates { paths: false, triggers: true, teleports: false, shapes: false }
    }

    pub fn teleports() -> Self {
        RenderUpdates { paths: false, triggers: false, teleports: true, shapes: false }
    }

    pub fn shapes() -> Self {
        RenderUpdates { paths: false, triggers: false, teleports: false, shapes: true }
    }

    pub fn or(&mut self, other: RenderUpdates) {
        self.paths |= other.paths;
        self.triggers |= other.triggers;
        self.teleports |= other.teleports;
        self.shapes |= other.shapes;
    }
}

fn process_events() {
    let mut loop_events : VecDeque<UIEvent> = VecDeque::new();

    let mut events = EVENTS.lock().unwrap();

    let mut event_list = events.clone();
    events.clear();

    drop(events);

    while let Some(event) = event_list.pop_front() {
        match event {
            UIEvent::DeletePath { path_id } => {
                PATHLOG.lock().unwrap().delete_path(path_id);
                loop_events.push_back(UIEvent::RenderUpdate { update: RenderUpdates::paths() });
            }
            UIEvent::ChangeDirectMode { new } => {
                PATHLOG.lock().unwrap().set_direct_mode(new);
            }
            UIEvent::ChangeAutosave { new } => {
                PATHLOG.lock().unwrap().set_autosave(new);
            }
            UIEvent::ChangeAutoReset { new } => {
                PATHLOG.lock().unwrap().set_autoreset(new);
            }
            UIEvent::SpawnTrigger { index, position, rotation } => {
                let trigger_size = CONFIG_STATE.lock().unwrap().trigger_sizes[index];
                if PATHLOG.lock().unwrap().is_empty() {
                    PATHLOG.lock().unwrap().create_trigger(index, position, rotation, trigger_size);
                    EVENTS.lock().unwrap().push_back(UIEvent::SpawnTeleport { index: ui::TeleportIndex::Main { i: index } });
                }
                else {
                    // TODO: popup warning
                }
            }
            UIEvent::DeleteTrigger { id } => {
                let pos = PATHLOG.lock().unwrap().checkpoint_triggers.iter().position(|t| t.id() == id);
                if let Some(i) = pos {
                    PATHLOG.lock().unwrap().checkpoint_triggers.remove(i);
                    continue;
                }

                let mut main_triggers = PATHLOG.lock().unwrap().main_triggers;
                let mut main_teleports = UISTATE.lock().unwrap().main_teleports;

                for t in 0..2 {
                    if let Some(trigger) = main_triggers[t] {
                        if trigger.id() == id {
                            main_triggers[t] = None;
                            main_teleports[t] = None;
                        }
                    }
                }

                PATHLOG.lock().unwrap().main_triggers = main_triggers;
                UISTATE.lock().unwrap().main_teleports = main_teleports;
            }
            UIEvent::StartRecording => {
                PATHLOG.lock().unwrap().start();
            }
            UIEvent::StopRecording => {
                let mut pathlog = PATHLOG.lock().unwrap();

                let recording_path_id = pathlog.recording_path.id();
                pathlog.mute_paths.insert(recording_path_id, false);
                pathlog.solo_paths.insert(recording_path_id, false);
                pathlog.stop();

                drop(pathlog);

                loop_events.push_back(UIEvent::RenderUpdate { update: RenderUpdates::paths() });
            }
            UIEvent::ResetRecording => {
                PATHLOG.lock().unwrap().reset();
            }
            UIEvent::ClearTriggers => {
                PATHLOG.lock().unwrap().clear_triggers();
                UISTATE.lock().unwrap().main_teleports = [None; 2];
            }
            UIEvent::CreateCollection => {
                PATHLOG.lock().unwrap().create_collection();
            }
            UIEvent::RenameCollection { id, new_name } => {
                PATHLOG.lock().unwrap().rename_collection(id, new_name);
            }
            UIEvent::DeleteCollection { id } => {
                PATHLOG.lock().unwrap().delete_collection(id);
                loop_events.push_back(UIEvent::RenderUpdate { update: RenderUpdates::paths() });
            }
            UIEvent::ToggleMute { id } => {
                let mut pathlog = PATHLOG.lock().unwrap();

                if let Some(b) = pathlog.mute_paths.get_mut(&id) { *b ^= true; }
                if let Some(b) = pathlog.mute_collections.get_mut(&id) { *b ^= true; }
                pathlog.update_visible();

                drop(pathlog);

                loop_events.push_back(UIEvent::RenderUpdate { update: RenderUpdates::paths() });
            },
            UIEvent::ToggleSolo { id } => {
                let mut pathlog = PATHLOG.lock().unwrap();

                if let Some(b) = pathlog.solo_paths.get_mut(&id) { *b ^= true; }
                if let Some(b) = pathlog.solo_collections.get_mut(&id) { *b ^= true; }
                pathlog.update_visible();

                drop(pathlog);

                loop_events.push_back(UIEvent::RenderUpdate { update: RenderUpdates::paths() });
            },
            UIEvent::ToggleActive { id } => {
                let mut pathlog = PATHLOG.lock().unwrap();

                if pathlog.active_collection == Some(id) {
                    pathlog.active_collection = None;
                }
                else {
                    pathlog.active_collection = Some(id);
                }

                drop(pathlog);
            }
            UIEvent::ToggleGoldFilter { collection_id } => {
                let mut pathlog = PATHLOG.lock().unwrap();

                if !pathlog.filters.contains_key(&collection_id) {
                    pathlog.filters.insert(collection_id, HighPassFilter::Gold);
                }
                else {
                    if let Some(HighPassFilter::Gold) = pathlog.filters.get(&collection_id) {
                        pathlog.filters.remove(&collection_id);
                    }
                    else {
                        *pathlog.filters.get_mut(&collection_id).unwrap() = HighPassFilter::Gold;
                    }
                }

                drop(pathlog);
            }
            UIEvent::SetPathFilter { collection_id, path_id } => {
                let mut pathlog = PATHLOG.lock().unwrap();

                match pathlog.filters.get_mut(&collection_id) {
                    Some(HighPassFilter::Path{ id }) => {
                        if *id == path_id {
                            pathlog.filters.remove(&collection_id);
                        }
                        else {
                            *pathlog.filters.get_mut(&collection_id).unwrap() = HighPassFilter::Path { id: path_id };
                        }
                    },
                    Some(HighPassFilter::Gold) => {
                        *pathlog.filters.get_mut(&collection_id).unwrap() = HighPassFilter::Path { id: path_id };
                    }
                    _ => {
                        pathlog.filters.insert(collection_id, HighPassFilter::Path{ id: path_id });
                    }
                }

                drop(pathlog);
            }
            UIEvent::SaveComparison => {
                let mut ui_state = UISTATE.lock().unwrap();

                if ui_state.file_path_rx.is_none() {
                    let (tx, rx) = mpsc::channel();
                    thread::spawn(move || {
                            tx.send(
                                FileDialog::new()
                                .add_filter("Celestial Comparison", &[FILE_EXTENTION])
                                .set_filename("Untitled")
                                .show_save_single_file()
                            ).unwrap();
                    });
                    ui_state.file_path_rx = Some(RX::Save { rx });
                    loop_events.push_back(UIEvent::SaveComparison);
                }
                else if let Some(RX::Save { rx }) = &ui_state.file_path_rx {
                    if let Ok(dialog_result) = rx.try_recv() {
                        drop(ui_state);

                        if let Ok(Some(path)) = dialog_result {
                            PATHLOG.lock().unwrap().save_comparison(path.to_str().unwrap().to_string());
                        }
                        UISTATE.lock().unwrap().file_path_rx = None;
                    }
                    else { loop_events.push_back(UIEvent::SaveComparison); }
                }
            }
            UIEvent::LoadComparison => {
                let mut ui_state = UISTATE.lock().unwrap();

                if ui_state.file_path_rx.is_none() {
                    let (tx, rx) = mpsc::channel();
                    thread::spawn(move || {
                        tx.send(
                            FileDialog::new()
                            .add_filter("Celestial Comparison", &[FILE_EXTENTION])
                            .add_filter("Any", &["*"])
                            .show_open_single_file()
                        ).unwrap();
                    });
                    ui_state.file_path_rx = Some(RX::Load { rx });
                    loop_events.push_back(UIEvent::LoadComparison);
                }
                else if let Some(RX::Load { rx }) = &ui_state.file_path_rx {
                    if let Ok(dialog_result) = rx.try_recv() {
                        drop(ui_state);

                        if let Ok(Some(path)) = dialog_result {
                            let load_res = PATHLOG.lock().unwrap().load_comparison(path.to_str().unwrap().to_string());
                            // if let Err(e) = PATHLOG.lock().unwrap().load_comparison(path.to_str().unwrap().to_string()) {
                            if let Err(e) = load_res {
                                UISTATE.lock().unwrap().file_path_rx = None;
                                error!("{e}");
                                continue;
                            }

                            if let Some(start_trigger) = PATHLOG.lock().unwrap().main_triggers[0] {
                                UISTATE.lock().unwrap().main_teleports[0] = Some(Teleport {
                                    location: start_trigger.position,
                                    rotation: start_trigger.rotation(),
                                    camera_rotation: None,
                                })
                            }

                            if let Some(end_trigger) = PATHLOG.lock().unwrap().main_triggers[1] {
                                UISTATE.lock().unwrap().main_teleports[1] = Some(Teleport {
                                    location: end_trigger.position,
                                    rotation: end_trigger.rotation(),
                                    camera_rotation: None,
                                })
                            }
                        }

                        UISTATE.lock().unwrap().file_path_rx = None;
                        loop_events.push_back(UIEvent::RenderUpdate { update: RenderUpdates::paths() });
                    }
                    else { loop_events.push_back(UIEvent::LoadComparison); }
                }
            }
            UIEvent::SaveConfig => {
                if let Err(e) = CONFIG_STATE.lock().unwrap().write(CONFIG_FILE_NAME.to_string()) {
                    error!("{e}");
                }
            },
            UIEvent::LoadConfig => {
                if let Err(e) = CONFIG_STATE.lock().unwrap().read(CONFIG_FILE_NAME.to_string()) {
                    error!("{e}");
                }
            },
            UIEvent::SelectPath { path_id, collection_id, modifier } => {
                let pathlog = PATHLOG.lock().unwrap();

                let collection = pathlog.get_collection(collection_id).unwrap().clone();
                let path = pathlog.path(&path_id).unwrap().clone();

                let mut selected = pathlog.selected_paths.get(&collection_id).unwrap().clone();

                drop(pathlog);

                match modifier {
                    1 => {
                        let last_id = *selected.last().unwrap_or(&(path.id()));
                        let mut last_pos = collection.paths().iter().position(|p| *p == last_id).unwrap();
                        let mut this_pos = collection.paths().iter().position(|p| *p == path.id()).unwrap();
                        if last_pos < this_pos { (last_pos, this_pos) = (this_pos + 1, last_pos + 1) }
                        for p in &collection.paths()[this_pos..last_pos] {
                            if let Some(pos) = selected.iter().position(|id| *id == *p) { selected.remove(pos);}
                            else { selected.push(*p) }
                        }
                    },
                    2 => {
                        if let Some(pos) = selected.iter().position(|id| *id == path.id()) { selected.remove(pos);}
                        else { selected.push(path.id()) }
                    },
                    _ => {
                        selected.clear();
                        selected.push(path.id());
                    }
                }

                PATHLOG.lock().unwrap().selected_paths.insert(collection_id, selected);
                loop_events.push_back(UIEvent::RenderUpdate { update: RenderUpdates::paths() });
            }
            UIEvent::Teleport { index } => {
                let t = match index {
                    TeleportIndex::Main { i } => {
                        UISTATE.lock().unwrap().main_teleports[i]
                    },
                    TeleportIndex::Extra { i } => {
                        UISTATE.lock().unwrap().extra_teleports[i]
                    },
                };

                if let Some(teleport) = t {
                    gamedata::teleport_player(teleport.location, teleport.rotation);
                    if let Some(cam_rotation) = teleport.camera_rotation {
                        gamedata::set_camera_rotation(cam_rotation);
                    }
                    loop_events.push_back(UIEvent::ResetRecording);
                }
            }
            UIEvent::SpawnTeleport { index } => {
                let teleport = Some(Teleport {
                    location: gamedata::get_player_position(),
                    rotation: gamedata::get_player_rotation(),
                    camera_rotation: Some(gamedata::get_camera_rotation()),
                });

                match index {
                    TeleportIndex::Main { i } => {
                        UISTATE.lock().unwrap().main_teleports[i] = teleport;
                    },
                    TeleportIndex::Extra { i } => {
                        UISTATE.lock().unwrap().extra_teleports[i] = teleport;
                    },
                }
            }
            UIEvent::RenderUpdate { update } => {
                RENDER_UPDATES.lock().unwrap().or(update);

                if update.paths {
                    PATHLOG.lock().unwrap().update_visible();
                }
            }
        }
    }

    EVENTS.lock().unwrap().append(&mut loop_events);
}

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
            render_path(pintar, RECORDING_GROUP.to_string(), &PATHLOG.lock().unwrap().recording_path, [1.0, 1.0, 1.0, 0.8], 0.02);

            pintar.clear_vertex_group(SHAPES_GROUP.to_string());
            render_custom_shapes(pintar);

            pintar.clear_vertex_group(TELEPORTS_GROUP.to_string());
            render_teleports(pintar);

            render_all_paths(pintar);

            pintar.clear_vertex_group(TRIGGERS_GROUP.to_string());
            render_triggers(pintar);

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
                            draw_debug(ui);
                        });
                    }
                    })
                .expect("successful render");

            process_events();
        }
    }

    // Call and return the result of the original method.
    ocular::get_present().expect("Uh oh. Present isn't hooked?!").call(this, sync_interval, flags)
}

fn render_path(pintar: &mut Pintar, vertex_group: String, path: &Path, color: [f32; 4], thickness: f32) {
    for segment in path.segments() {
        if segment.len() < 2 { continue; }
        pintar.add_line(vertex_group.clone(), segment, color, thickness);
    }
}

fn render_all_paths(pintar: &mut Pintar) {
    if !RENDER_UPDATES.lock().unwrap().paths { return; }

    RENDER_UPDATES.lock().unwrap().paths = false;

    pintar.clear_vertex_group(PATHS_GROUP.to_string());

    let pathlog = PATHLOG.lock().unwrap();

    let compared_paths = pathlog.compared_paths().clone();
    let ignored_paths = pathlog.ignored_paths().clone();
    let selected_paths = pathlog.selected_paths.clone();
    let comparison = pathlog.comparison();

    drop(pathlog);

    let config = CONFIG_STATE.lock().unwrap();

    let fast_color = config.fast_color;
    let slow_color = config.slow_color;
    let gold_color = config.gold_color;
    let select_color = config.select_color;

    drop(config);

    // let mut visible_collection = PathCollection::new("Visible".to_string());
    let mut selected : Vec<Uuid> = Vec::new();
    for s in selected_paths.values() {
        selected.extend(s);
    }

    for i in 0..compared_paths.len() {
        let (path_id, position) = compared_paths[i];

        let fast = fast_color;
        let slow = slow_color;
        let lerp = |a: f32, b: f32, t: f32| -> f32 { a * (1.0-t) + b * t };
        let mut color: [f32; 4];
        let mut thick: f32;

        if position == 0 {
            color = gold_color;
            thick = 0.04;
        }
        else if comparison.len == 2 {
            color = slow_color;
            thick = 0.02;
        }
        else {
            let p = (position - 1) as f32 / (comparison.len - 2) as f32;

            color = [
                lerp(fast[0], slow[0], p),
                lerp(fast[1], slow[1], p),
                lerp(fast[2], slow[2], p),
                lerp(fast[3], slow[3], p),
            ];
            thick = 0.02;
        }

        if matches!(comparison.mode, ComparisonMode::Median) {
            for ignored_id in &ignored_paths[i] {
                let ignored_color = [color[0], color[1], color[2], color[3] * 0.5];
                render_path(pintar, PATHS_GROUP.to_string(), &PATHLOG.lock().unwrap().path(&ignored_id).unwrap(), ignored_color, thick);
            }
        }

        if selected.contains(&path_id) {
            color = select_color;
            thick = 0.04;
        }

        let pathlog = PATHLOG.lock().unwrap();
        render_path(pintar, PATHS_GROUP.to_string(), &pathlog.path(&path_id).unwrap(), color, thick);
    }
}

fn render_triggers(pintar: &mut Pintar) {
    let pathlog = PATHLOG.lock().unwrap();

    let checkpoint_triggers = pathlog.checkpoint_triggers.clone();
    let main_triggers = pathlog.main_triggers;

    drop(pathlog);

    let config = CONFIG_STATE.lock().unwrap();

    let checkpoint_color = config.checkpoint_color;
    let trigger_colors = config.trigger_colors;

    drop(config);

    for collider in &checkpoint_triggers {
        pintar.add_default_mesh(TRIGGERS_GROUP.to_string(), pintar::primitives::cube::new(checkpoint_color).scale(collider.size).rotate(collider.rotation()).translate(collider.position));
    }

    for i in 0..2 {
        if let Some(collider) = main_triggers[i] {
            pintar.add_default_mesh(TRIGGERS_GROUP.to_string(), pintar::primitives::cube::new(trigger_colors[i]).scale(collider.size).rotate(collider.rotation()).translate(collider.position));
        }
    }
}

fn render_teleports(pintar: &mut Pintar) {
    let config = CONFIG_STATE.lock().unwrap();

    let accent_colors = config.accent_colors;

    drop(config);

    let ui_state = UISTATE.lock().unwrap();

    let teleports = ui_state.extra_teleports;

    drop(ui_state);

    let lerp = |a: u8, b: u8, t: f32| -> f32 { (a as f32 * (1.0-t) + b as f32 * t) / 255. };

    for i in 0..10 {
        if let Some(teleport) = &teleports[i] {
            let pos = teleport.location;

            let p = i as f32 / 10.;
            let mut color = [
                lerp(accent_colors[0][0], accent_colors[1][0], p),
                lerp(accent_colors[0][1], accent_colors[1][1], p),
                lerp(accent_colors[0][2], accent_colors[1][2], p),
                lerp(accent_colors[0][3], accent_colors[1][3], p),
            ];

            // info!("DEBUG: color: {color:?}");
            // let mut color = trigger_colors[0];
            // color[3] = 0.5;
            // pos[1] += 1.0;
            pintar.add_default_mesh(TELEPORTS_GROUP.to_string(), pintar::primitives::cylinder::new(color).scale([0.6, 0.05, 0.6]).translate(pos));
            color[3] *= 0.25;
            pintar.add_default_mesh(TELEPORTS_GROUP.to_string(), pintar::primitives::cylinder::new(color).scale([0.5, 0.052, 0.5]).translate(pos));
        }
    }

    // if let Some(teleport) = &teleports[0] {
    //     let pos = teleport.location;
    //     let mut color = trigger_colors[0];
    //     // color[3] = 0.5;
    //     // pos[1] += 1.0;
    //     pintar.add_default_mesh(TELEPORTS_GROUP.to_string(), pintar::primitives::cylinder::new(color).scale([0.6, 0.05, 0.6]).translate(pos));
    //     color[3] *= 0.25;
    //     pintar.add_default_mesh(TELEPORTS_GROUP.to_string(), pintar::primitives::cylinder::new(color).scale([0.5, 0.052, 0.5]).translate(pos));
    // }

    // if let Some(teleport) = &teleports[1] {
    //     let pos = teleport.location;
    //     let mut color = trigger_colors[1];
    //     // color[3] = 0.5;
    //     // pos[1] += 1.0;
    //     pintar.add_default_mesh(TELEPORTS_GROUP.to_string(), pintar::primitives::cylinder::new(color).scale([0.6, 0.05, 0.6]).translate(pos));
    //     color[3] *= 0.25;
    //     pintar.add_default_mesh(TELEPORTS_GROUP.to_string(), pintar::primitives::cylinder::new(color).scale([0.5, 0.052, 0.5]).translate(pos));
    // }
}

// fn render_custom_shapes(pintar: &mut Pintar, egui: &UIState) {
fn render_custom_shapes(pintar: &mut Pintar) {
    let ui_state = UISTATE.lock().unwrap();

    let custom_shapes = ui_state.custom_shapes.clone();

    drop(ui_state);

    for shape in &custom_shapes {
        if shape.1 { continue; }
        match shape.0.shape_type {
            ShapeType::Box => {
                pintar.add_default_mesh(SHAPES_GROUP.to_string(), pintar::primitives::cube::new(shape.0.color.to_rgba_premultiplied())
                    .scale(shape.0.size)
                    .rotate(shape.0.rotation)
                    .translate(shape.0.position));
            }
            ShapeType::Sphere => {
                let mut size = shape.0.size;
                size[1] = size[0];
                size[2] = size[0];
                pintar.add_default_mesh(SHAPES_GROUP.to_string(), pintar::primitives::sphere::new(shape.0.color.to_rgba_premultiplied())
                    .scale(size)
                    .translate(shape.0.position));
            }
            ShapeType::Cylinder => {
                let mut size = shape.0.size;
                size[2] = size[0];
                pintar.add_default_mesh(SHAPES_GROUP.to_string(), pintar::primitives::cylinder::new(shape.0.color.to_rgba_premultiplied())
                    .scale(size)
                    .translate(shape.0.position));
            }
        }
    }
}

fn draw_debug(ui: &mut egui::Ui) {
    ui.add(egui::Label::new(
        RichText::new(format!("{:?}", unsafe { IS_POINTER_OVER_EGUI }))
    ).selectable(false));
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