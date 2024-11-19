use std::ffi::c_void;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, VecDeque};
use pintar::mesh::DefaultMesh;
use windows::core::HRESULT;
use windows::Win32::System::Console::AllocConsole;
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use windows::Win32::Foundation::{BOOL, HMODULE, HWND, LRESULT, WPARAM, LPARAM};
use windows::Win32::Graphics::Dxgi::{IDXGISwapChain, DXGI_SWAP_CHAIN_DESC};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;
use windows::Win32::Graphics::Direct3D11::{ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11DepthStencilView};
use windows::Win32::UI::WindowsAndMessaging::{SetWindowLongPtrA, CallWindowProcW, WNDPROC, GWLP_WNDPROC, WM_KEYDOWN};

use tracing::*;
use tracing_appender::*;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use glam::{Vec3, Mat3};
use egui::{Color32, Event, RichText};
use egui_directx11::DirectX11Renderer;
use egui_win32::InputManager;
use egui_keybind::{Bind, Keybind, Shortcut};
use uuid::Uuid;
use native_dialog::FileDialog;
use lazy_static::lazy_static;

mod tether;
pub mod gamedata;
pub mod config;
pub mod pathlog;

use config::*;
use pathlog::*;

use ocular;
use pintar::Pintar;

// static TICKRATE : u32 = 60;

static mut PATHLOG : Option<PathLog> = None;
static mut CONFIG_STATE : Option<ConfigState> = None;
static mut EGUI_STATE : Option<UIState> = None;

static mut PINTAR : Option<Pintar> = None;
static mut EGUI_RENDERER : Option<DirectX11Renderer> = None;
static mut INPUT_MANAGER : Option<InputManager> = None;
static mut OLD_WNDPROC : Option<WNDPROC> = None;

// lazy_static! {
//     static ref PATHLOG : Arc<Mutex<PathLog>> = Arc::new(Mutex::new(PathLog::new()));
//     static ref CONFIG_STATE : Arc<Mutex<ConfigState>> = Arc::new(Mutex::new(ConfigState::new()));
//     static ref EGUI_STATE : Arc<Mutex<UIState>> = Arc::new(Mutex::new(UIState::new()));
// }

enum RX {
    Save { rx: mpsc::Receiver<Result<Option<PathBuf>, native_dialog::Error>> },
    Load { rx: mpsc::Receiver<Result<Option<PathBuf>, native_dialog::Error>> },
}

#[derive(PartialEq)]
enum Tab { Comparison, Paths, Config, Credits }

enum UIEvent {
    RemovePath {
        path_id: Uuid,
        collection_id: Uuid,
    },
    ChangeDirectMode {
        new: bool,
    },
    ChangeAutosave {
        new: bool,
    },
    SpawnTrigger {
        index: usize,
        position: [f32; 3],
        rotation: [f32; 3],
        size: [f32; 3]
    },
    StartRecording,
    StopRecording,
    ResetRecording,
    CreateCollection,
    RenameCollection {
        id: Uuid,
        new_name: String,
    },
    DeleteCollection {
        id: Uuid,
    },
    ToggleCollection {
        id: Uuid,
    },
    SaveComparison,
    LoadComparison,
    // SelectPath {
    //     id: u64,
    //     collection: String,
    //     modifier: u8,
    // },
}
struct UIState {
    events: VecDeque<UIEvent>,
    file_path_rx: Option<RX>,
    tab: Tab,
    modifier: u8,
    delete_mode: bool,
    renaming_collection: Option<Uuid>,
    renaming_name: String,
    selected_paths: HashMap<Uuid, Vec<Uuid>>,
    mute_paths: HashMap<Uuid, bool>,
    solo_paths: HashMap<Uuid, bool>,
    mute_collections: HashMap<Uuid, bool>,
    solo_collections: HashMap<Uuid, bool>,

    // colors: [egui::ecolor::Hsva; 7],
}

impl UIState {
    pub fn new() -> UIState {
        let mut state = UIState {
            events: VecDeque::new(),
            file_path_rx: None,
            tab: Tab::Comparison,
            modifier: 0,
            delete_mode: false,
            renaming_collection: None,
            renaming_name: "".to_string(),
            selected_paths: HashMap::new(),
            mute_paths: HashMap::new(),
            solo_paths: HashMap::new(),
            mute_collections: HashMap::new(),
            solo_collections: HashMap::new(),
        };

        unsafe {
            state.selected_paths.insert(PATHLOG.as_ref().unwrap().direct_paths.id(), Vec::new());
        }

        state
    }

    pub unsafe fn process_events(&mut self) {
        let pathlog = PATHLOG.as_mut().unwrap();
        
        let mut loop_events : VecDeque<UIEvent> = VecDeque::new();

        while let Some(event) = self.events.pop_front() {
            match event {
                UIEvent::RemovePath { path_id, collection_id } => {
                    pathlog.remove(path_id, collection_id);
                }
                UIEvent::ChangeDirectMode { new } => {
                    pathlog.set_direct_mode(new);
                }
                UIEvent::ChangeAutosave { new } => {
                    pathlog.set_autosave(new);
                }
                UIEvent::SpawnTrigger { index, position, rotation, size } => {
                    let mut ok = true;
                    for collection in &pathlog.path_collections {
                        ok = collection.paths().is_empty();
                    }
                    if ok {
                        pathlog.create_trigger(index, position, rotation, size);
                    }
                    else {
                        // TODO warning
                    }
                }
                UIEvent::StartRecording => {
                    pathlog.start();
                }
                UIEvent::StopRecording => {
                    self.mute_paths.insert(pathlog.recording_path.id(), false);
                    self.solo_paths.insert(pathlog.recording_path.id(), false);
                    pathlog.stop();
                }
                UIEvent::ResetRecording => {
                    pathlog.reset();
                }
                UIEvent::CreateCollection => {
                    let new_collection = PathCollection::new(DEFAULT_COLLECTION_NAME.to_string());
                    self.mute_collections.insert(new_collection.id(), false);
                    self.solo_collections.insert(new_collection.id(), false);
                    self.selected_paths.insert(new_collection.id(), Vec::new());
                    pathlog.path_collections.push(new_collection);
                }
                UIEvent::RenameCollection { id, mut new_name } => {
                    for i in 0..pathlog.path_collections.len() {
                        if pathlog.path_collections[i].id() == id {
                            if new_name == "" { new_name = "can't put nothing bro".to_string() }
                            pathlog.path_collections[i].name = new_name.clone();
                        }
                    }
                }
                UIEvent::DeleteCollection { id } => {
                    if let Some(index) = pathlog.path_collections.iter().position(|c| c.id() == id) {
                        pathlog.path_collections.remove(index);
                    }
                }
                UIEvent::ToggleCollection { id } => {
                    if let Some(index) = pathlog.active_collections.iter().position(|x| *x == id) {
                        pathlog.active_collections.remove(index);
                    }
                    else {
                        pathlog.active_collections.push(id);
                    }
                }
                UIEvent::SaveComparison => {
                    if self.file_path_rx.is_none() {
                        let (tx, rx) = mpsc::channel();
                        thread::spawn(move || {
                                tx.send(FileDialog::new().show_save_single_file()).unwrap();
                        });
                        self.file_path_rx = Some(RX::Save { rx });
                        loop_events.push_back(UIEvent::SaveComparison);
                    }
                    else if let Some(RX::Save { rx }) = &self.file_path_rx {
                        if let Ok(dialog_result) = rx.try_recv() {
                            if let Ok(Some(path)) = dialog_result { pathlog.save_comparison(path.to_str().unwrap().to_string()); }
                            self.file_path_rx = None;
                        }
                        else { loop_events.push_back(UIEvent::SaveComparison); }
                    }
                }
                UIEvent::LoadComparison => {
                    if self.file_path_rx.is_none() {
                        let (tx, rx) = mpsc::channel();
                        thread::spawn(move || {
                            tx.send(FileDialog::new().show_open_single_file()).unwrap();
                        });
                        self.file_path_rx = Some(RX::Load { rx });
                        loop_events.push_back(UIEvent::LoadComparison);
                    }
                    else if let Some(RX::Load { rx }) = &self.file_path_rx {
                        if let Ok(dialog_result) = rx.try_recv() {
                            if let Ok(Some(path)) = dialog_result { pathlog.load_comparison(path.to_str().unwrap().to_string()); }
                            self.file_path_rx = None;
                        }
                        else { loop_events.push_back(UIEvent::LoadComparison); }
                    }
                }
            }
        }

        self.events.append(&mut loop_events);
    }
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
    unsafe {
        init_globals(&this);
    
        if let Some(pintar) = PINTAR.as_mut() {
            let pathlog = PATHLOG.as_mut().unwrap();
            let config = CONFIG_STATE.as_ref().unwrap();
            let state = EGUI_STATE.as_mut().unwrap();

            pathlog.update(&gamedata::get_player_position(), &gamedata::get_player_rotation());

            pintar.set_default_view_proj(gamedata::get_view_matrix());

            render_path(&pathlog.recording_path, pintar, [1.0, 1.0, 1.0, 0.8], 0.02);

            let mut visible_collection = PathCollection::new("Visible".to_string());
            let mut selected : Vec<Uuid> = Vec::new();

            for collection in &pathlog.path_collections {
                if !state.solo_collections.contains_key(&collection.id()) { state.solo_collections.insert(collection.id(), false); }
                if !state.mute_collections.contains_key(&collection.id()) { state.mute_collections.insert(collection.id(), false); }
                if !state.selected_paths.contains_key(&collection.id()) { state.selected_paths.insert(collection.id(), Vec::new()); }

                let mut visible = true;
                for v in state.solo_collections.values() {
                    if *v {visible = false;}
                }
                if state.solo_collections.get(&collection.id()) == Some(&true) { visible = true; }
                if state.mute_collections.get(&collection.id()) == Some(&true) { visible = false; }
                if !visible { continue; }

                for path in collection.paths() {
                    if visible_collection.paths().iter().position(|p| p.id() == path.id()).is_some() { continue; }

                    visible_collection.add(path.clone()); // mmh :/

                    if state.selected_paths.get(&collection.id()).unwrap().contains(&path.id()) {
                        selected.push(path.id());
                    }
                }
            }

            let visible_paths = visible_collection.paths();
            for i in 0..visible_paths.len() {
                let path = &visible_paths[i];

                if !state.mute_paths.contains_key(&path.id()) { state.mute_paths.insert(path.id(), false); }
                if !state.solo_paths.contains_key(&path.id()) { state.solo_paths.insert(path.id(), false); }

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

                render_path(&path, pintar, color, thick);
            }

            for path in pathlog.direct_paths.paths() {
                if !state.mute_paths.contains_key(&path.id()) { state.mute_paths.insert(path.id(), false); }
                if !state.solo_paths.contains_key(&path.id()) { state.solo_paths.insert(path.id(), false); }

                let mut visible = true;
                for v in state.solo_paths.values() {
                    if *v {visible = false;}
                }
                if state.solo_paths.get(&path.id()) == Some(&true) { visible = true; }
                if state.mute_paths.get(&path.id()) == Some(&true) { visible = false; }
                if !visible { continue; }
                render_path(&path, pintar, [1.0, 1.0, 1.0, 1.0], 0.02);
            }

            for i in 0..2 {
                if let Some(collider) = pathlog.triggers[i] {
                    pintar.add_default_mesh(pintar::primitives::cube::new(config.trigger_color[i]).scale(collider.size()).rotate(collider.rotation()).translate(collider.position()));
                }
            }

            pintar.render();
            pintar.clear_vertex_groups();
        }

        if let Some(dx_renderer) = EGUI_RENDERER.as_mut() {
            let input = match INPUT_MANAGER {
                Some(ref mut input_manager) => input_manager.collect_input().unwrap(),
                None => egui::RawInput::default(),
            };

            check_input(&input);

            let mut egui_state = EGUI_STATE.as_mut().unwrap();

            dx_renderer
                .paint(&this, &mut egui_state, input.clone(), |ctx, state| {
                    egui::Window::new("Celestial")
                        .default_size(egui::vec2(300f32, 300f32))
                        .vscroll(true)
                        .resizable(true)
                        .min_size([300.0, 300.0])
                        .frame(egui::Frame::window(&ctx.style()).inner_margin(7.0))
                        .show(ctx, |ui| {
                            draw_ui(ui);
                        });
                    
                    egui::Window::new("Timer")
                        .resizable(false)
                        .title_bar(false)
                        .frame(egui::Frame::window(&ctx.style()).inner_margin(7.0))
                        .show(ctx, |ui| {
                            draw_timer(ui);
                        });
                    })
                .expect("successful render");

            egui_state.process_events();
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

fn render_path(path: &Path, pintar: &mut Pintar, color: [f32; 4], thickness: f32) {
    if path.len() < 2 { return; }

    pintar.add_line(path.get_node(0), path.get_node(1), color, thickness);

    for node in &path.get_nodes()[2..] {
        pintar.extend_line(*node, color, thickness);
    }
}

fn draw_ui(ui: &mut egui::Ui) {
    unsafe {
        let state = EGUI_STATE.as_mut().unwrap();
        let config = CONFIG_STATE.as_ref().unwrap();

        ui.style_mut().visuals.selection.bg_fill = config.accent_colors[0];
        ui.spacing_mut().item_spacing = egui::vec2(15.0, 3.0);
        // ui.spacing_mut().window_margin = egui::Margin::same(50.0);

        ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
            ui.selectable_value(&mut state.tab, Tab::Comparison, egui::RichText::new("Comparison").strong());
            ui.selectable_value(&mut state.tab, Tab::Paths, egui::RichText::new("Paths").strong());
            ui.selectable_value(&mut state.tab, Tab::Config, egui::RichText::new("Config").strong());
            ui.selectable_value(&mut state.tab, Tab::Credits, egui::RichText::new("Credits").strong());
        });

        // ui.separator();

        ui.spacing_mut().item_spacing = egui::vec2(10.0, 3.0);

        draw_comparison_tab(ui);
        draw_paths_tab(ui);
        draw_config_tab(ui);
        draw_credits_tab(ui);
    }
}

fn draw_timer(ui: &mut egui::Ui) {
    unsafe {
        let pathlog = PATHLOG.as_ref().unwrap();
        let config = CONFIG_STATE.as_ref().unwrap();

        let time = pathlog.time();
        ui.add(egui::Label::new(
            RichText::new(format!("{:02}:{:02}.{:03}", time / 60000, (time % 60000) / 1000, (time % 1000)))
            .size(config.timer_size)
        ).selectable(false));
    }
}

unsafe fn draw_comparison_tab(ui: &mut egui::Ui) {
    let pathlog = PATHLOG.as_ref().unwrap();
    let config = CONFIG_STATE.as_ref().unwrap();
    let state = EGUI_STATE.as_mut().unwrap();

    if state.tab != Tab::Comparison { return; }
    ui.separator();

    if ui.interact_bg(egui::Sense::click()).clicked() {
        state.renaming_collection = None;
    }

    for collection in &pathlog.path_collections {
        egui::Grid::new(collection.id().to_string() + "buttons")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            // if ui.interact_bg(egui::Sense::click()).clicked() {
            //     state.renaming_collection = None;
            // }

            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                if state.renaming_collection == Some(collection.id()) {
                    let response = ui.add(egui::TextEdit::singleline(&mut state.renaming_name).desired_width(200.0));
                    if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let new_name = state.renaming_name.clone();
                        state.events.push_back(UIEvent::RenameCollection { id: collection.id(), new_name });
                        state.renaming_collection = None;
                    }
                }
                else if ui.label(collection.name.clone()).clicked() {
                    state.renaming_collection = Some(collection.id());
                    state.renaming_name = collection.name.clone();
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if state.delete_mode {
                    if ui.add(egui::Button::new("\u{1F5D9}").min_size(egui::vec2(19.0, 19.0))).clicked() {
                        state.events.push_back(UIEvent::DeleteCollection { id: collection.id() });
                    }
                }

                let original_hovered_weak_bg_fill = ui.style_mut().visuals.widgets.hovered.weak_bg_fill;
                let original_inactive_weak_bg_fill = ui.style_mut().visuals.widgets.inactive.weak_bg_fill;

                // ui.style_mut().visuals.widgets.hovered.weak_bg_fill = config.accent_colors[0].gamma_multiply(1.2);
                let mut solo_button_text = egui::RichText::new("\u{1F1F8}");

                if *state.solo_collections.get(&collection.id()).unwrap() {
                    ui.style_mut().visuals.widgets.hovered.weak_bg_fill = config.accent_colors[0].gamma_multiply(1.2);
                    ui.style_mut().visuals.widgets.inactive.weak_bg_fill = config.accent_colors[0];
                    solo_button_text = solo_button_text.strong();
                }

                if ui.add(egui::Button::new(solo_button_text).min_size(egui::vec2(19.0, 19.0))).clicked() {
                    *state.solo_collections.get_mut(&collection.id()).unwrap() ^= true;
                }

                ui.style_mut().visuals.widgets.hovered.weak_bg_fill = original_hovered_weak_bg_fill;
                ui.style_mut().visuals.widgets.inactive.weak_bg_fill = original_inactive_weak_bg_fill;

                let mut mute_button_text = egui::RichText::new("\u{1F1F2}");
                if *state.mute_collections.get(&collection.id()).unwrap() {
                    ui.style_mut().visuals.widgets.hovered.weak_bg_fill = config.accent_colors[0].gamma_multiply(1.2);
                    ui.style_mut().visuals.widgets.inactive.weak_bg_fill = config.accent_colors[0];
                    mute_button_text = mute_button_text.strong();
                }

                if ui.add(egui::Button::new(mute_button_text).min_size(egui::vec2(19.0, 19.0))).clicked() {
                    *state.mute_collections.get_mut(&collection.id()).unwrap() ^= true;
                }

                ui.style_mut().visuals.widgets.hovered.weak_bg_fill = original_hovered_weak_bg_fill;
                ui.style_mut().visuals.widgets.inactive.weak_bg_fill = original_inactive_weak_bg_fill;

                let mut arm_button_text = egui::RichText::new("\u{2B55}");
                if pathlog.active_collections.contains(&collection.id()) {
                    ui.style_mut().visuals.widgets.hovered.weak_bg_fill = config.accent_colors[1].gamma_multiply(1.2);
                    ui.style_mut().visuals.widgets.inactive.weak_bg_fill = config.accent_colors[1];
                    arm_button_text = arm_button_text.strong();
                }

                if ui.add(
                    egui::Button::new(arm_button_text)
                        .min_size(egui::vec2(19.0, 19.0))
                        .rounding(egui::Rounding::same(10.0))
                    ).clicked() {
                    state.events.push_back(UIEvent::ToggleCollection { id: collection.id() });
                }

                ui.style_mut().visuals.widgets.hovered.weak_bg_fill = original_hovered_weak_bg_fill;
                ui.style_mut().visuals.widgets.inactive.weak_bg_fill = original_inactive_weak_bg_fill;
            });
            ui.end_row();
        });

        egui::CollapsingHeader::new("").id_source(collection.id().to_string() + "collapsing")
        .show(ui, |ui| {
            egui::Grid::new(collection.id().to_string() + "paths")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(false)
                .with_row_color(|i, _style| {
                    if i < collection.paths().len() && collection.paths()[i].id() == pathlog.latest_path {
                        Some(egui::Color32::from_gray(54))
                    }
                    else { None }
                })
                .show(ui, |ui| {
                    if ui.interact_bg(egui::Sense::click()).clicked() {
                        state.selected_paths.get_mut(&collection.id()).unwrap().clear();
                        state.renaming_collection = None;
                    }
            
                    for path in collection.paths() {
                        let original_bg_color = ui.style_mut().visuals.widgets.noninteractive.bg_fill;
                        // if path.id() == pathlog.latest_path {
                            ui.style_mut().visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(255, 0, 255);
                        // }
                        draw_path(path, &collection, ui);
                        ui.style_mut().visuals.widgets.noninteractive.bg_fill = original_bg_color;
                    }
                });
            });

        ui.separator();
    }

    egui::Grid::new("saveload+")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                if ui.add(egui::Button::new("Save").min_size(egui::vec2(19.0, 19.0))).clicked() {
                    state.events.push_back(UIEvent::SaveComparison);
                }
                if ui.add(egui::Button::new("Load").min_size(egui::vec2(19.0, 19.0))).clicked() {
                    state.events.push_back(UIEvent::LoadComparison);
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.add(egui::Button::new("\u{2796}").min_size(egui::vec2(19.0, 19.0))).clicked() {
                    state.delete_mode ^= true;
                }
                if ui.add(egui::Button::new("\u{2795}").min_size(egui::vec2(19.0, 19.0))).clicked() {
                    state.events.push_back(UIEvent::CreateCollection);
                }
            });
            ui.end_row();
        });
}

unsafe fn draw_paths_tab(ui: &mut egui::Ui) {
    let pathlog = PATHLOG.as_ref().unwrap();
    let state = EGUI_STATE.as_mut().unwrap();

    if state.tab != Tab::Paths { return; }
    ui.separator();

    egui::Grid::new("paths_grid")
    .num_columns(2)
    .spacing([40.0, 4.0])
    .striped(true)
    .show(ui, |ui| {
        if ui.interact_bg(egui::Sense::click()).clicked() {
            state.selected_paths.get_mut(&pathlog.direct_paths.id()).unwrap().clear();
        }

        for path in pathlog.direct_paths.paths() {
            draw_path(path, &pathlog.direct_paths, ui);
        }
    });
}

unsafe fn draw_path(path: &Path, collection: &PathCollection, ui: &mut egui::Ui) {
    let state = EGUI_STATE.as_mut().unwrap();
    let config = CONFIG_STATE.as_mut().unwrap();

    if !state.mute_paths.contains_key(&path.id()) { state.mute_paths.insert(path.id(), false); }
    if !state.solo_paths.contains_key(&path.id()) { state.solo_paths.insert(path.id(), false); }

    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {

        let original_hovered_weak_bg_fill = ui.style_mut().visuals.widgets.hovered.weak_bg_fill;
        let original_inactive_weak_bg_fill = ui.style_mut().visuals.widgets.inactive.weak_bg_fill;

        let mut mute_button_text = egui::RichText::new("\u{1F1F2}");

        if *state.mute_paths.get(&path.id()).unwrap() {
            ui.style_mut().visuals.widgets.hovered.weak_bg_fill = config.accent_colors[0].gamma_multiply(1.2);
            ui.style_mut().visuals.widgets.inactive.weak_bg_fill = config.accent_colors[0];
            mute_button_text = mute_button_text.strong();
        }

        if ui.add(egui::Button::new(mute_button_text).min_size(egui::vec2(19.0, 19.0))).clicked() {
            *state.mute_paths.get_mut(&path.id()).unwrap() ^= true;
        }

        ui.style_mut().visuals.widgets.hovered.weak_bg_fill = original_hovered_weak_bg_fill;
        ui.style_mut().visuals.widgets.inactive.weak_bg_fill = original_inactive_weak_bg_fill;

        let mut solo_button_text = egui::RichText::new("\u{1F1F8}");

        if *state.solo_paths.get(&path.id()).unwrap() {
            ui.style_mut().visuals.widgets.hovered.weak_bg_fill = config.accent_colors[0].gamma_multiply(1.2);
            ui.style_mut().visuals.widgets.inactive.weak_bg_fill = config.accent_colors[0];
            solo_button_text = solo_button_text.strong();
        }

        if ui.add(egui::Button::new(solo_button_text).min_size(egui::vec2(19.0, 19.0))).clicked() {
            *state.solo_paths.get_mut(&path.id()).unwrap() ^= true;
        }

        ui.style_mut().visuals.widgets.hovered.weak_bg_fill = original_hovered_weak_bg_fill;
        ui.style_mut().visuals.widgets.inactive.weak_bg_fill = original_inactive_weak_bg_fill;

        let mods = state.modifier;
        let selected = state.selected_paths.get_mut(&collection.id()).unwrap();
        if selected.contains(&path.id()) { ui.style_mut().visuals.override_text_color = Some(config.select_color.as_color32()); }

        let time = path.get_time();
        if ui.add(egui::Label::new(format!("{:02}:{:02}.{:03}", time / 60000, (time % 60000) / 1000, (time % 1000))).selectable(true)).clicked() {
            match mods {
                1 => {
                    let last_id = *selected.last().unwrap_or(&(path.id()));
                    let mut last_pos = collection.paths().iter().position(|p| p.id() == last_id).unwrap();
                    let mut this_pos = collection.paths().iter().position(|p| p.id() == path.id()).unwrap();
                    if last_pos < this_pos { (last_pos, this_pos) = (this_pos, last_pos) }
                    for p in &collection.paths()[this_pos..last_pos] {
                        if let Some(pos) = selected.iter().position(|id| *id == p.id()) { selected.remove(pos);} // TODO: for some reason shift select only works upwards
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
        }

        ui.style_mut().visuals.override_text_color = None;
    });

    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
        if state.delete_mode {
            if ui.add(egui::Button::new("\u{1F5D9}").min_size(egui::vec2(19.0, 19.0))).clicked() {
                state.selected_paths.get_mut(&collection.id()).unwrap().clear();
                state.events.push_back(UIEvent::RemovePath { path_id: path.id(), collection_id: collection.id() });
            }
        }
    });

    ui.end_row();
}

unsafe fn draw_config_tab(ui: &mut egui::Ui) {
    let config = CONFIG_STATE.as_mut().unwrap();
    let state = EGUI_STATE.as_mut().unwrap();

    if state.tab != Tab::Config { return; }
    ui.separator();

    ui.add_space(5.0);
    egui::Grid::new("toggles_grid")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label("Direct Recording Mode");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if toggle_switch(ui, &mut config.direct_mode).clicked() {
                    state.events.push_back(UIEvent::ChangeDirectMode { new: config.direct_mode });
                }
            });
            ui.end_row();

            ui.label("Comparison Autosave");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if toggle_switch(ui, &mut config.autosave).clicked() {
                    state.events.push_back(UIEvent::ChangeAutosave { new: config.autosave });
                }
            });
            ui.end_row();

            ui.label("Start Trigger Size");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(egui::DragValue::new(&mut config.trigger_size[0][2]).speed(0.1).clamp_range(0.1..=10.0));
                ui.add(egui::DragValue::new(&mut config.trigger_size[0][1]).speed(0.1).clamp_range(0.1..=10.0));
                ui.add(egui::DragValue::new(&mut config.trigger_size[0][0]).speed(0.1).clamp_range(0.1..=10.0));
            });
            ui.end_row();

            ui.label("End Trigger Size");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(egui::DragValue::new(&mut config.trigger_size[1][2]).speed(0.1).clamp_range(0.1..=10.0));
                ui.add(egui::DragValue::new(&mut config.trigger_size[1][1]).speed(0.1).clamp_range(0.1..=10.0));
                ui.add(egui::DragValue::new(&mut config.trigger_size[1][0]).speed(0.1).clamp_range(0.1..=10.0));
            });
            ui.end_row();

            ui.label("Timer Size");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(egui::DragValue::new(&mut config.timer_size).speed(0.5).clamp_range(6.9..=69.0));
            });
            ui.end_row();
        });

    ui.add_space(20.0);
    ui.heading("Keybinds");
    ui.separator();

    egui::Grid::new("credits_grid")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label("Toggle window");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(Keybind::new(&mut config.toggle_window_keybind, "toggle_window_keybind"));
            });
            ui.end_row();

            if config.direct_mode {
                ui.label("Start recording");
            }
            else {
                ui.label("Spawn start trigger");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(Keybind::new(&mut config.start_keybind, "start_keybind"));
            });
            ui.end_row();

            if config.direct_mode {
                ui.label("Stop recording");
            }
            else {
                ui.label("Spawn end trigger");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(Keybind::new(&mut config.stop_keybind, "stop_keybind"));
            });
            ui.end_row();

            ui.label("Reset recording");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(Keybind::new(&mut config.reset_keybind, "reset_keybind"));
            });
            ui.end_row();
        });

    ui.add_space(20.0);
    ui.heading("Colors");
    ui.separator();

    egui::Grid::new("colors_grid")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Path Rendering").size(15.0));
            ui.end_row();

            ui.label("Start Trigger");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut tmp_color = config.trigger_color[0].as_hsva();
                if ui.color_edit_button_hsva(&mut tmp_color).changed() {
                    config.trigger_color[0] = tmp_color.to_rgba_premultiplied();
                }
            });
            ui.end_row();

            ui.label("End Trigger");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut tmp_color = config.trigger_color[1].as_hsva();
                if ui.color_edit_button_hsva(&mut tmp_color).changed() {
                    config.trigger_color[1] = tmp_color.to_rgba_premultiplied();
                }
            });
            ui.end_row();

            ui.label("Path Gradient: Fast");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut tmp_color = config.fast_color.as_hsva();
                if ui.color_edit_button_hsva(&mut tmp_color).changed() {
                    config.fast_color = tmp_color.to_rgba_premultiplied();
                }
            });
            ui.end_row();

            ui.label("Path Gradient: Slow");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut tmp_color = config.slow_color.as_hsva();
                if ui.color_edit_button_hsva(&mut tmp_color).changed() {
                    config.slow_color = tmp_color.to_rgba_premultiplied();
                }
            });
            ui.end_row();

            ui.label("Fastest Path");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut tmp_color = config.gold_color.as_hsva();
                if ui.color_edit_button_hsva(&mut tmp_color).changed() {
                    config.gold_color = tmp_color.to_rgba_premultiplied();
                }
            });
            ui.end_row();

            ui.label("Selected Path");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut tmp_color = config.select_color.as_hsva();
                if ui.color_edit_button_hsva(&mut tmp_color).changed() {
                    config.select_color = tmp_color.to_rgba_premultiplied();
                }
            });
            ui.end_row();

            ui.end_row();
            ui.label(egui::RichText::new("UI").size(15.0));
            ui.end_row();

            ui.label("Accent 0");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut tmp_color = config.accent_colors[0].as_hsva();
                if ui.color_edit_button_hsva(&mut tmp_color).changed() {
                    config.accent_colors[0] = tmp_color.as_color32();
                }
            });
            ui.end_row();

            ui.label("Accent 1");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut tmp_color = config.accent_colors[1].as_hsva();
                if ui.color_edit_button_hsva(&mut tmp_color).changed() {
                    config.accent_colors[1] = tmp_color.as_color32();
                }
            });
            ui.end_row();
        });

    ui.separator();

    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
        if ui.add(egui::Button::new("Save")).clicked() {
            config.write("celestial.ini".to_string());
        }
        if ui.add(egui::Button::new("Load")).clicked() {
            if let Err(e) = config.read("celestial.ini".to_string()) {
                println!("{e}"); // ini error doesn't implement tracing::Value. maybe change this
            }
        }
    });
}

unsafe fn draw_credits_tab(ui: &mut egui::Ui) {
    let state = EGUI_STATE.as_ref().unwrap();

    if state.tab != Tab::Credits { return; }

    ui.set_min_width(300.0);
    ui.separator();

    egui::Grid::new("credits_grid")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.hyperlink_to("Hellbufl", "https://github.com/Hellbufl");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("Author"); });
            ui.end_row();
            ui.hyperlink_to("Woeful_Wolf", "https://github.com/WoefulWolf");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
                ui.label("modules");
                ui.hyperlink_to("UI", "https://github.com/WoefulWolf/egui-directx");
                ui.label("and");
                ui.hyperlink_to("Graphics", "https://github.com/WoefulWolf/pintar");
                ui.label(", ");
                ui.hyperlink_to("Hooking", "https://github.com/WoefulWolf/ocular-rs");
                ui.spacing_mut().item_spacing = egui::vec2(15.0, 3.0);
            });
            ui.end_row();
        });
}

fn toggle_switch(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let desired_size = ui.spacing().interact_size.y * egui::vec2(2.0, 1.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| {
        // egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, "")
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on)
    });

    if ui.is_rect_visible(rect) {
        // let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
        let how_on = ui.ctx().animate_bool(response.id, *on);
        let visuals = ui.style().interact_selectable(&response, *on);
        let rect = rect.expand(visuals.expansion);
        let radius = 0.5 * rect.height();
        ui.painter()
            .rect(rect, radius, visuals.bg_fill, visuals.bg_stroke);
        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = egui::pos2(circle_x, rect.center().y);
        ui.painter()
            .circle(center, 0.75 * radius, visuals.bg_fill, visuals.fg_stroke);
    }

    response
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

fn check_input(input: &egui::RawInput) {
    let events = &input.events;
    unsafe {
        let config = CONFIG_STATE.as_mut().unwrap();
        let state = EGUI_STATE.as_mut().unwrap();

        // let mut config = CONFIG_STATE.lock().unwrap();
        // let mut state = EGUI_STATE.lock().unwrap();

        state.modifier = 0;

        if input.modifiers.shift {
            state.modifier = 1;
        }

        if input.modifiers.ctrl {
            state.modifier = 2;
        }

        for e in events {
            // if config.toggle_window_keybind.compare_to_event(e) {
            //     config.show_ui ^= true;
            // }

            if config.start_keybind.compare_to_event(e) {
                if config.direct_mode {
                    state.events.push_back(UIEvent::StartRecording);
                } else {
                    state.events.push_back(UIEvent::SpawnTrigger {
                        index: 0,
                        position: gamedata::get_player_position(),
                        rotation: gamedata::get_player_rotation(),
                        size: config.trigger_size[0],
                    });
                }
            }

            if config.stop_keybind.compare_to_event(e) {
                if config.direct_mode {
                    state.events.push_back(UIEvent::StopRecording);
                } else {
                    state.events.push_back(UIEvent::SpawnTrigger {
                        index: 1,
                        position: gamedata::get_player_position(),
                        rotation: gamedata::get_player_rotation(),
                        size: config.trigger_size[1],
                    });
                }
            }

            if config.reset_keybind.compare_to_event(e) {
                state.events.push_back(UIEvent::ResetRecording);
            }
        }
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
        PATHLOG = Some(PathLog::new());
        CONFIG_STATE = Some(ConfigState::new());
        EGUI_STATE = Some(UIState::new());
    }

    ocular::hook_present(hk_present);
    ocular::hook_resize_buffers(hk_resize_buffers);
    ocular::hook_om_set_render_targets(hk_om_set_render_targets);

    // let tick_duration = (1000 / TICKRATE) as u128;
    // let mut last_tick = Instant::now();

    // loop {
    //     if last_tick.elapsed().as_millis() < tick_duration { continue; }
    //     last_tick = Instant::now();

    //     EGUI_STATE.lock().unwrap().process_events();
    //     PATHLOG.lock().unwrap().update(&gamedata::get_player_position(), &gamedata::get_player_rotation());
    // }
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

// TODO LIST

// - timer !
// - popup messages
// - move paths between collections
// - should you be able to duplicate paths?
// - teleporation