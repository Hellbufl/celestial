use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use uuid::Uuid;
use native_dialog::FileDialog;
use egui_keybind::Keybind;

use crate::config::{ConfigState, AsColor32, AsHsva};
use crate::pathlog::{PathLog, PathCollection, Path};
use crate::gamedata;

pub const DEFAULT_COLLECTION_NAME : &str = "New Collection";

enum RX {
    Save { rx: mpsc::Receiver<Result<Option<PathBuf>, native_dialog::Error>> },
    Load { rx: mpsc::Receiver<Result<Option<PathBuf>, native_dialog::Error>> },
}

#[derive(PartialEq)]
pub enum Tab { Comparison, Paths, Config, Credits }

pub enum UIEvent {
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

pub struct UIState {
    pub events: VecDeque<UIEvent>,
    file_path_rx: Option<RX>,
    tab: Tab,
    pub modifier: u8,
    delete_mode: bool,
    renaming_collection: Option<Uuid>,
    renaming_name: String,
    pub selected_paths: HashMap<Uuid, Vec<Uuid>>,
    pub mute_paths: HashMap<Uuid, bool>,
    pub solo_paths: HashMap<Uuid, bool>,
    pub mute_collections: HashMap<Uuid, bool>,
    pub solo_collections: HashMap<Uuid, bool>,

    // colors: [egui::ecolor::Hsva; 7],
}

impl UIState {
    pub fn new() -> UIState {
        let state = UIState {
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

        state
    }

    pub unsafe fn process_events(&mut self, pathlog: &mut PathLog) {
        let mut loop_events : VecDeque<UIEvent> = VecDeque::new();

        while let Some(event) = self.events.pop_front() {
            match event {
                UIEvent::RemovePath { path_id, collection_id } => {
                    self.mute_paths.remove(&path_id);
                    self.solo_paths.remove(&path_id);
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
                        self.mute_collections.remove(&id);
                        self.solo_collections.remove(&id);
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

pub fn check_input(input: &egui::RawInput, egui: &mut UIState, config: &mut ConfigState) {
    let events = &input.events;
    
    egui.modifier = 0;

    if input.modifiers.shift {
        egui.modifier = 1;
    }

    if input.modifiers.ctrl {
        egui.modifier = 2;
    }

    for e in events {
        // if config.toggle_window_keybind.compare_to_event(e) {
        //     config.show_ui ^= true;
        // }

        if config.start_keybind.compare_to_event(e) {
            if config.direct_mode {
                egui.events.push_back(UIEvent::StartRecording);
            } else {
                egui.events.push_back(UIEvent::SpawnTrigger {
                    index: 0,
                    position: gamedata::get_player_position(),
                    rotation: gamedata::get_player_rotation(),
                    size: config.trigger_size[0],
                });
            }
        }

        if config.stop_keybind.compare_to_event(e) {
            if config.direct_mode {
                egui.events.push_back(UIEvent::StopRecording);
            } else {
                egui.events.push_back(UIEvent::SpawnTrigger {
                    index: 1,
                    position: gamedata::get_player_position(),
                    rotation: gamedata::get_player_rotation(),
                    size: config.trigger_size[1],
                });
            }
        }

        if config.reset_keybind.compare_to_event(e) {
            egui.events.push_back(UIEvent::ResetRecording);
        }
    }
}


pub fn draw_ui(ui: &mut egui::Ui, state: &mut UIState, config: &mut ConfigState, pathlog: &mut PathLog) {
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

    draw_comparison_tab(ui, state, config, pathlog);
    draw_paths_tab(ui, state, config, pathlog);
    draw_config_tab(ui, state, config);
    draw_credits_tab(ui, state);
}

pub fn draw_timer(ui: &mut egui::Ui, config: &mut ConfigState, pathlog: &mut PathLog) {
    let time = pathlog.time();
    ui.add(egui::Label::new(
        egui::RichText::new(format!("{:02}:{:02}.{:03}", time / 60000, (time % 60000) / 1000, (time % 1000)))
        .size(config.timer_size)
    ).selectable(false));
}

fn draw_comparison_tab(ui: &mut egui::Ui, state: &mut UIState, config: &mut ConfigState, pathlog: &PathLog) {
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
                // .with_row_color(|i, _style| {
                //     if i < collection.paths().len() && collection.paths()[i].id() == pathlog.latest_path {
                //         Some(egui::Color32::from_gray(54))
                //     }
                //     else { None }
                // })
                .show(ui, |ui| {
                    if ui.interact_bg(egui::Sense::click()).clicked() {
                        state.selected_paths.get_mut(&collection.id()).unwrap().clear();
                        state.renaming_collection = None;
                    }
            
                    for path in collection.paths() {
                        draw_path(ui, state, config, path, &collection);
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

fn draw_paths_tab(ui: &mut egui::Ui, state: &mut UIState, config: &ConfigState, pathlog: &PathLog) {
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
            draw_path(ui, state, config, path, &pathlog.direct_paths);
        }
    });
}

// TODO: select paths through events
fn draw_path(ui: &mut egui::Ui, state: &mut UIState, config: &ConfigState, path: &Path, collection: &PathCollection) {
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

        let time = path.time();
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

fn draw_config_tab(ui: &mut egui::Ui, state: &mut UIState, config: &mut ConfigState) {
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

    egui::Grid::new("config_grid")
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

fn draw_credits_tab(ui: &mut egui::Ui, state: &mut UIState) {
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

            ui.label("Aloyark");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("Testing"); });
            ui.end_row();

            ui.label("Icarus");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("Testing"); });
            ui.end_row();

            ui.label("Percy");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("Testing"); });
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