#![allow(static_mut_refs)]

use std::collections::VecDeque;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::Instant;
use glam::Vec3;
use native_dialog::FileDialog;
use windows::core::HRESULT;
use windows::Win32::System::Console::AllocConsole;
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use windows::Win32::Foundation::{BOOL, HMODULE, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dxgi::{IDXGIDevice, IDXGISurface, IDXGISwapChain, DXGI_SURFACE_DESC, DXGI_SWAP_CHAIN_DESC};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;
use windows::Win32::Graphics::Direct3D11::{ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11DepthStencilView};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, GetClientRect, SetWindowLongPtrA,
    GWLP_WNDPROC, WM_MOUSEMOVE, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WNDPROC,
};

use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::Devices::HumanInterfaceDevice::{DirectInput8Create, IDirectInput8A, IDirectInputDevice8A, GUID_SysMouse, GUID_SysKeyboard};
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
static TRIGGERS_GROUP : &str = "triggers";
static TELEPORTS_GROUP : &str = "teleports";
static SHAPES_GROUP : &str = "custom_shapes";

static SCREEN_DIMENSIONS: Lazy<Mutex<ScreenDimensions>> = Lazy::new(|| Mutex::new(ScreenDimensions::default()));

pub static GLOBAL_STATE: Lazy<Mutex<GlobalState>> = Lazy::new(|| Mutex::new(GlobalState::init()));
// static mut GLOBAL_STATE : Option<GlobalState> = None;
// static mut DEBUG_STATE : Option<DebugState> = None;

// static PINTAR: Lazy<Mutex<Option<Pintar>>> = Lazy::new(|| Mutex::new(None));

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

enum RX {
    Save { rx: mpsc::Receiver<Result<Option<PathBuf>, native_dialog::Error>> },
    Load { rx: mpsc::Receiver<Result<Option<PathBuf>, native_dialog::Error>> },
}

pub struct RenderUpdates {
    paths: bool,
    triggers: bool,
    teleports: bool,
    shapes: bool,
}

impl RenderUpdates {
    pub fn new() -> Self {
        RenderUpdates { paths: false, triggers: false, teleports: false, shapes: false }
    }
}

struct GlobalState {
    pathlog: PathLog,
    config: ConfigState,
    egui: UIState,
    events: VecDeque<UIEvent>,
    updates: RenderUpdates,
}

impl GlobalState {
    pub fn init() -> GlobalState {
        GlobalState {
            pathlog: PathLog::init(),
            config: ConfigState::init(),
            egui: UIState::init(),
            events: VecDeque::new(),
            updates: RenderUpdates { paths: false, triggers: false, teleports: false, shapes: false },
        }
    }

    pub unsafe fn process_events(&mut self) {
        let mut loop_events : VecDeque<UIEvent> = VecDeque::new();

        while let Some(event) = self.events.pop_front() {
            match event {
                UIEvent::DeletePath { path_id, collection_id } => {
                    self.egui.mute_paths.remove(&path_id);
                    self.egui.solo_paths.remove(&path_id);
                    self.pathlog.remove(path_id, collection_id);
                    self.updates.paths = true;
                }
                UIEvent::ChangeDirectMode { new } => {
                    self.pathlog.set_direct_mode(new);
                }
                UIEvent::ChangeAutosave { new } => {
                    self.pathlog.set_autosave(new);
                }
                UIEvent::ChangeAutoReset { new } => {
                    self.pathlog.set_autoreset(new);
                }
                UIEvent::SpawnTrigger { index, position, rotation } => {
                     if self.pathlog.is_empty() {
                        self.pathlog.create_trigger(index, position, rotation, self.config.trigger_size[index]);
                        self.events.push_back(UIEvent::SpawnTeleport { index });
                    }
                    else {
                        // TODO: popup warning
                    }
                }
                UIEvent::StartRecording => {
                    self.pathlog.start();
                }
                UIEvent::StopRecording => {
                    self.egui.mute_paths.insert(self.pathlog.recording_path.id(), false);
                    self.egui.solo_paths.insert(self.pathlog.recording_path.id(), false);
                    self.pathlog.stop();
                    self.updates.paths = true;
                }
                UIEvent::ResetRecording => {
                    self.pathlog.reset();
                }
                UIEvent::ClearTriggers => {
                    self.pathlog.clear_triggers();
                }
                UIEvent::CreateCollection => {
                    let new_collection = PathCollection::new(DEFAULT_COLLECTION_NAME.to_string());
                    self.egui.mute_collections.insert(new_collection.id(), false);
                    self.egui.solo_collections.insert(new_collection.id(), false);
                    self.egui.selected_paths.insert(new_collection.id(), Vec::new());
                    self.pathlog.path_collections.push(new_collection);
                }
                UIEvent::RenameCollection { id, mut new_name } => {
                    for i in 0..self.pathlog.path_collections.len() {
                        if self.pathlog.path_collections[i].id() == id {
                            if new_name == "" { new_name = "can't put nothing bro".to_string() }
                            self.pathlog.path_collections[i].name = new_name.clone();
                        }
                    }
                }
                UIEvent::DeleteCollection { id } => {
                    if let Some(index) = self.pathlog.path_collections.iter().position(|c| c.id() == id) {

                        for path in self.pathlog.path_collections[index].paths() {
                            self.egui.mute_paths.remove(&path.id());
                            self.egui.solo_paths.remove(&path.id());
                        }

                        self.egui.mute_collections.remove(&id);
                        self.egui.solo_collections.remove(&id);
                        self.pathlog.path_collections.remove(index);
                        self.updates.paths = true;
                    }
                }
                UIEvent::ToggleActive { id } => {
                    if self.pathlog.active_collection == Some(id) {
                        self.pathlog.active_collection = None;
                    }
                    else {
                        self.pathlog.active_collection = Some(id);
                    }
                }
                UIEvent::ToggleGoldFilter { collection_id } => {
                    if !self.pathlog.filters.contains_key(&collection_id) {
                        self.pathlog.filters.insert(collection_id, HighPassFilter::GOLD);
                    }
                    else {
                        if let Some(HighPassFilter::GOLD) = self.pathlog.filters.get(&collection_id) {
                            self.pathlog.filters.remove(&collection_id);
                        }
                        else {
                            *self.pathlog.filters.get_mut(&collection_id).unwrap() = HighPassFilter::GOLD;
                        }
                    }
                }
                UIEvent::SetPathFilter { collection_id, path_id } => {
                    match self.pathlog.filters.get_mut(&collection_id) {
                        Some(HighPassFilter::PATH{ id }) => {
                            if *id == path_id {
                                self.pathlog.filters.remove(&collection_id);
                            }
                            else {
                                *self.pathlog.filters.get_mut(&collection_id).unwrap() = HighPassFilter::PATH { id: path_id };
                            }
                        },
                        Some(HighPassFilter::GOLD) => {
                            *self.pathlog.filters.get_mut(&collection_id).unwrap() = HighPassFilter::PATH { id: path_id };
                        }
                        _ => {
                            self.pathlog.filters.insert(collection_id, HighPassFilter::PATH{ id: path_id });
                        }
                    }
                }
                UIEvent::SaveComparison => {
                    if self.egui.file_path_rx.is_none() {
                        let (tx, rx) = mpsc::channel();
                        thread::spawn(move || {
                                tx.send(FileDialog::new().show_save_single_file()).unwrap();
                        });
                        self.egui.file_path_rx = Some(RX::Save { rx });
                        loop_events.push_back(UIEvent::SaveComparison);
                    }
                    else if let Some(RX::Save { rx }) = &self.egui.file_path_rx {
                        if let Ok(dialog_result) = rx.try_recv() {
                            if let Ok(Some(path)) = dialog_result { self.pathlog.save_comparison(path.to_str().unwrap().to_string()); }
                            self.egui.file_path_rx = None;
                        }
                        else { loop_events.push_back(UIEvent::SaveComparison); }
                    }
                }
                UIEvent::LoadComparison => {
                    if self.egui.file_path_rx.is_none() {
                        let (tx, rx) = mpsc::channel();
                        thread::spawn(move || {
                            tx.send(FileDialog::new().show_open_single_file()).unwrap();
                        });
                        self.egui.file_path_rx = Some(RX::Load { rx });
                        loop_events.push_back(UIEvent::LoadComparison);
                    }
                    else if let Some(RX::Load { rx }) = &self.egui.file_path_rx {
                        if let Ok(dialog_result) = rx.try_recv() {
                            if let Ok(Some(path)) = dialog_result {
                                if let Err(e) = self.pathlog.load_comparison(path.to_str().unwrap().to_string()) {
                                    error!("{e}");
                                    continue;
                                }

                                self.egui.mute_collections.clear();
                                self.egui.solo_collections.clear();
                                self.egui.selected_paths.clear();
                                self.egui.mute_paths.clear();
                                self.egui.solo_paths.clear();

                                for collection in &self.pathlog.path_collections {
                                    self.egui.mute_collections.insert(collection.id(), false);
                                    self.egui.solo_collections.insert(collection.id(), false);
                                    self.egui.selected_paths.insert(collection.id(), Vec::new());

                                    for path in collection.paths() {
                                        self.egui.mute_paths.insert(path.id(), false);
                                        self.egui.solo_paths.insert(path.id(), false);
                                    }
                                }

                                if let Some(start_trigger) = self.pathlog.main_triggers[0] {
                                    self.egui.teleports[0] = Some(Teleport {
                                        location: start_trigger.position,
                                        rotation: start_trigger.rotation(),
                                        camera_rotation: None,
                                    })
                                }

                                if let Some(end_trigger) = self.pathlog.main_triggers[1] {
                                    self.egui.teleports[1] = Some(Teleport {
                                        location: end_trigger.position,
                                        rotation: end_trigger.rotation(),
                                        camera_rotation: None,
                                    })
                                }
                            }
                            self.egui.file_path_rx = None;
                            self.updates.paths = true;
                        }
                        else { loop_events.push_back(UIEvent::LoadComparison); }
                    }
                }
                UIEvent::SelectPath { path_id, collection_id, modifier } => {
                    let collection = &self.pathlog.path_collections[self.pathlog.path_collections.iter().position(|c| c.id() == collection_id).unwrap()];
                    let path = collection.get_path(path_id).unwrap();

                    let selected = self.egui.selected_paths.get_mut(&collection.id()).unwrap();

                    match modifier {
                        1 => {
                            let last_id = *selected.last().unwrap_or(&(path.id()));
                            let mut last_pos = collection.paths().iter().position(|p| p.id() == last_id).unwrap();
                            let mut this_pos = collection.paths().iter().position(|p| p.id() == path.id()).unwrap();
                            if last_pos < this_pos { (last_pos, this_pos) = (this_pos + 1, last_pos + 1) }
                            for p in &collection.paths()[this_pos..last_pos] {
                                if let Some(pos) = selected.iter().position(|id| *id == p.id()) { selected.remove(pos);}
                                else { selected.push(p.id()) }
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
                    self.updates.paths = true;
                }
                UIEvent::Teleport { index } => {
                    if let Some(teleport) = &self.egui.teleports[index] {
                        gamedata::teleport_player(teleport.location, teleport.rotation);
                        if let Some(cam_rotation) = teleport.camera_rotation {
                            gamedata::set_camera_rotation(cam_rotation);
                        }
                        loop_events.push_back(UIEvent::ResetRecording);
                    }
                }
                UIEvent::SpawnTeleport { index } => {
                    if index > 1 { continue; }
                    self.egui.teleports[index] = Some(Teleport {
                        location: gamedata::get_player_position(),
                        rotation: gamedata::get_player_rotation(),
                        camera_rotation: Some(gamedata::get_camera_rotation()),
                    })
                }
            }
        }

        self.events.append(&mut loop_events);
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

    if let Ok(mut dims) = SCREEN_DIMENSIONS.lock() {
        let mut client_rect = RECT::default();
        if unsafe { GetClientRect(sd.OutputWindow, &mut client_rect).is_ok() } {
            dims.window_size.0 = (client_rect.right - client_rect.left) as u32;
            dims.window_size.1 = (client_rect.bottom - client_rect.top) as u32;
        }
    }

    if PINTAR.is_none() {
        let mut pintar = Pintar::new(&this, 0);
        // pintar.add_line_vertex_group("paths".to_string());
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
    let frame_start = Instant::now();

    unsafe {
        init_globals(&this);

        // let debug = DEBUG_STATE.as_mut().unwrap();

        // let elapsed = debug.last_frame.elapsed().as_secs_f32();

        // let speed = (Vec3::from_array(gamedata::get_player_position()) - debug.last_player_pos).length() / elapsed;
        // debug.player_speeds[debug.frame_count % 60] = speed;
        // // debug.player_speeds.push(speed);
        // // if debug.calc_avg {
        // if debug.frame_count % 10 == 0 {
        //     debug.average_player_speed = debug.player_speeds.iter().sum::<f32>() / debug.player_speeds.len() as f32;
        //     // debug.player_speeds.clear();
        //     // debug.calc_avg = false;
        // }

        // debug.last_player_pos = gamedata::get_player_position().into();
        // debug.last_frame = Instant::now();
        // debug.frame_count += 1;

        if let Ok(mut state) = GLOBAL_STATE.lock() {
            if gamedata::get_is_loading() {
                state.pathlog.pause();
            } else {
                state.pathlog.unpause();
            }

            state.pathlog.update(&gamedata::get_player_position(), &gamedata::get_player_rotation());
        }

        // let view_proj = gamedata::get_view_matrix();


        // let mut global_state = GLOBAL_STATE.as_mut().unwrap();

        if let Some(pintar) = PINTAR.as_mut() {
            // DEBUG_STATE.as_mut().unwrap().copy_time = 0;

            // let egui = &mut global_state.egui;
            // let pathlog = &mut global_state.pathlog;
            // let config = &global_state.config;
            // let updates = &mut global_state.updates;
            // let events = &mut GLOBAL_STATE.as_mut().unwrap().events;

            // if gamedata::get_is_loading() {
            //     pathlog.pause();
            // } else {
            //     pathlog.unpause();
            // }

            // pathlog.update(&gamedata::get_player_position(), &gamedata::get_player_rotation(), updates);

            let view_proj = gamedata::get_view_matrix();

            pintar.set_default_view_proj(view_proj);

            let triggers_vertex_group: &mut pintar::vertex_group::VertexGroup<pintar::default_elements::DefaultVertex, pintar::default_elements::DefaultConstants> = pintar.get_vertex_group_as(TRIGGERS_GROUP.to_string()).unwrap();
            triggers_vertex_group.constants.view_proj = XMMatrix::from(&view_proj);

            let teleports_vertex_group: &mut pintar::vertex_group::VertexGroup<pintar::default_elements::DefaultVertex, pintar::default_elements::DefaultConstants> = pintar.get_vertex_group_as(TELEPORTS_GROUP.to_string()).unwrap();
            teleports_vertex_group.constants.view_proj = XMMatrix::from(&view_proj);

            let shapes_vertex_group: &mut pintar::vertex_group::VertexGroup<pintar::default_elements::DefaultVertex, pintar::default_elements::DefaultConstants> = pintar.get_vertex_group_as(SHAPES_GROUP.to_string()).unwrap();
            shapes_vertex_group.constants.view_proj = XMMatrix::from(&view_proj);

            if let Ok(state) = GLOBAL_STATE.lock() {
                render_path(pintar, &state.pathlog.recording_path, [1.0, 1.0, 1.0, 0.8], 0.02);
            }

            pintar.clear_vertex_group(SHAPES_GROUP.to_string());
            render_custom_shapes(pintar);
            // render_custom_shapes(pintar, egui);

            pintar.clear_vertex_group(TELEPORTS_GROUP.to_string());
            render_teleports(pintar);
            // render_teleports(pintar, egui, config);

            render_all_paths(pintar);
            // if updates.paths {
            //     pintar.clear_vertex_group("default_line".to_string());
            //     render_all_paths(pintar, egui, config, pathlog);
            //     updates.paths = false;
            // }

            pintar.clear_vertex_group(TRIGGERS_GROUP.to_string());
            render_triggers(pintar);
            // render_triggers(pintar, config, pathlog);

            pintar.render();
            // pintar.clear_all_vertex_groups();
        }

        if let Some(dx_renderer) = EGUI_RENDERER.as_mut() {
            let input = match INPUT_MANAGER {
                Some(ref mut input_manager) => input_manager.collect_input().unwrap(),
                None => egui::RawInput::default(),
            };

            // let mut global_state = GLOBAL_STATE.as_mut().unwrap();

            ui::check_input(&input);
            // ui::check_input(&input, &mut global_state.egui, &mut global_state.config);

            // let mut global_state = GLOBAL_STATE.lock().unwrap();

            // i have to pass in some reference idk
            let mut nothing = 0;

            dx_renderer
                .paint(&this, &mut nothing, input.clone(), |ctx, s| {
                    // ctx.set_zoom_factor(state.config.zoom);

                    // my best attempt at getting a fucking window position but ofc it's private
                    // let state = ctx.memory(|mem| mem.areas().get(id).copied());

                    egui::Window::new("Celestial")
                        .default_size(egui::vec2(300f32, 300f32))
                        .vscroll(true)
                        .resizable(true)
                        .min_size([300.0, 300.0])
                        .frame(egui::Frame::window(&ctx.style()).inner_margin(7.0))
                        .show(ctx, |ui| {
                            IS_POINTER_OVER_EGUI = ctx.is_pointer_over_area();
                            EGUI_WANTS_KEYBOARD_INPUT = ctx.wants_keyboard_input();
                            ui::draw_ui(ui);
                            // ui::draw_ui(ui, &mut state.egui, &mut state.config, &mut state.pathlog);
                        });

                    egui::Window::new("Timer")
                        .resizable(false)
                        .title_bar(false)
                        .frame(egui::Frame::window(&ctx.style()).inner_margin(7.0))
                        .show(ctx, |ui| {
                            ui::draw_timer(ui);
                            // ui::draw_timer(ui, &mut state.config, &mut state.pathlog);
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

            if let Ok(mut state) = GLOBAL_STATE.lock() {
                // state.egui.process_events(&mut state);
                state.process_events();
            }
        }

        // DEBUG_STATE.as_mut().unwrap().frame_time = frame_start.elapsed().as_micros() as u64;
    }

    // Call and return the result of the original method.
    ocular::get_present().expect("Uh oh. Present isn't hooked?!").call(this, sync_interval, flags)
}

fn render_path(pintar: &mut Pintar, path: &Path, color: [f32; 4], thickness: f32) {
    for segment in path.segments() {
        if segment.len() < 2 { continue; }
        pintar.add_line("default_line".to_string(), segment, color, thickness);
    }
}

// fn render_all_paths(pintar: &mut Pintar, egui: &UIState, config: &ConfigState, pathlog: &PathLog) {
fn render_all_paths(pintar: &mut Pintar) {
    let state = GLOBAL_STATE.lock().unwrap();

    if !state.updates.paths { return; }

    pintar.clear_vertex_group("default_line".to_string());

    let mut visible_collection = PathCollection::new("Visible".to_string());
    let mut selected : Vec<Uuid> = Vec::new();

    for collection in &state.pathlog.path_collections {
        let mut visible = true;
        for v in state.egui.solo_collections.values() {
            if *v {visible = false;}
        }

        if !visible { continue; }

        for path in collection.paths() {
            if visible_collection.paths().contains(path) { continue; }

            visible_collection.add(path.clone(), None);

            if state.egui.selected_paths.get(&collection.id()).unwrap().contains(&path.id()) {
                selected.push(path.id());
            }
        }
    }

    let visible_paths = visible_collection.paths();
    for i in 0..visible_paths.len() {
        let path = &visible_paths[i];

        let mut visible = true;
        for v in state.egui.solo_paths.values() {
            if *v {visible = false;}
        }
        if state.egui.solo_paths.get(&path.id()) == Some(&true) { visible = true; }
        if state.egui.mute_paths.get(&path.id()) == Some(&true) { visible = false; }
        if !visible { continue; }

        let fast = state.config.fast_color;
        let slow = state.config.slow_color;
        let lerp = |a: f32, b: f32, t: f32| -> f32 { a * (1.0-t) + b * t };
        let mut color: [f32; 4];
        let thick: f32;

        if i == 0 {
            color = state.config.gold_color;
            thick = 0.04;
        }
        else if visible_paths.len() == 2 {
            color = state.config.slow_color;
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
            color = state.config.select_color;
        }

        render_path(pintar, &path, color, thick);
    }
}

// fn render_triggers(pintar: &mut Pintar, config: &ConfigState, pathlog: &PathLog) {
fn render_triggers(pintar: &mut Pintar) {
    let state = GLOBAL_STATE.lock().unwrap();

    for collider in &state.pathlog.checkpoint_triggers {
        pintar.add_default_mesh(TRIGGERS_GROUP.to_string(), pintar::primitives::cube::new(state.config.checkpoint_color).scale(collider.size).rotate(collider.rotation()).translate(collider.position));
    }

    for i in 0..2 {
        if let Some(collider) = state.pathlog.main_triggers[i] {
            pintar.add_default_mesh(TRIGGERS_GROUP.to_string(), pintar::primitives::cube::new(state.config.trigger_color[i]).scale(collider.size).rotate(collider.rotation()).translate(collider.position));
        }
    }
}

// fn render_teleports(pintar: &mut Pintar, egui: &UIState, config: &ConfigState) {
fn render_teleports(pintar: &mut Pintar) {
    let state = GLOBAL_STATE.lock().unwrap();

    if let Some(teleport) = &state.egui.teleports[0] {
        let pos = teleport.location;
        let mut color = state.config.trigger_color[0];
        // color[3] = 0.5;
        // pos[1] += 1.0;
        pintar.add_default_mesh(TELEPORTS_GROUP.to_string(), pintar::primitives::cylinder::new(color).scale([0.6, 0.05, 0.6]).translate(pos));
        color[3] *= 0.25;
        pintar.add_default_mesh(TELEPORTS_GROUP.to_string(), pintar::primitives::cylinder::new(color).scale([0.5, 0.052, 0.5]).translate(pos));
    }

    if let Some(teleport) = &state.egui.teleports[1] {
        let pos = teleport.location;
        let mut color = state.config.trigger_color[1];
        // color[3] = 0.5;
        // pos[1] += 1.0;
        pintar.add_default_mesh(TELEPORTS_GROUP.to_string(), pintar::primitives::cylinder::new(color).scale([0.6, 0.05, 0.6]).translate(pos));
        color[3] *= 0.25;
        pintar.add_default_mesh(TELEPORTS_GROUP.to_string(), pintar::primitives::cylinder::new(color).scale([0.5, 0.052, 0.5]).translate(pos));
    }
}

// fn render_custom_shapes(pintar: &mut Pintar, egui: &UIState) {
fn render_custom_shapes(pintar: &mut Pintar) {
    let state = GLOBAL_STATE.lock().unwrap();

    for shape in &state.egui.custom_shapes {
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
        RichText::new(format!("{:?}", gamedata::get_is_loading()))
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
        // GLOBAL_STATE = Some(GlobalState::init());
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