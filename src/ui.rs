use std::collections::VecDeque;
use std::f32::consts::PI;
use egui::epaint::Hsva;
use egui::Color32;
use uuid::Uuid;
use egui_keybind::Keybind;

use crate::config::{self, AsColor32, AsHsva, CompareKeybindToEvent};
use crate::pathdata::HighPassFilter;
use crate::{gamedata, pathlog, CONFIG_STATE, EVENTS, PATHLOG, RX, UISTATE};

#[derive(Clone, Copy, PartialEq)]
pub enum Tab { Comparison, Paths, Triggers, Config, Credits, CustomShapes }

#[derive(Clone, Copy)]
pub struct Teleport {
    pub location: [f32; 3],
    pub rotation: [f32; 3],
    pub camera_rotation: Option<[f32; 2]>,
}

#[derive(Clone)]
pub enum UIEvent {
    DeletePath {
        path_id: Uuid,
    },
    ChangeDirectMode {
        new: bool,
    },
    ChangeAutosave {
        new: bool,
    },
    ChangeAutoReset {
        new: bool,
    },
    SpawnTrigger {
        index: usize,
        position: [f32; 3],
        rotation: [f32; 3],
    },
    StartRecording,
    StopRecording,
    ResetRecording,
    ClearTriggers,
    CreateCollection,
    RenameCollection {
        id: Uuid,
        new_name: String,
    },
    DeleteCollection {
        id: Uuid,
    },
    ToggleMute {
        id: Uuid,
    },
    ToggleSolo {
        id: Uuid,
    },
    ToggleActive {
        id: Uuid,
    },
    ToggleGoldFilter {
        collection_id: Uuid,
    },
    SetPathFilter {
        collection_id: Uuid,
        path_id: Uuid,
    },
    SaveComparison,
    LoadComparison,
    SaveConfig,
    LoadConfig,
    SelectPath {
        path_id: Uuid,
        collection_id: Uuid,
        modifier: u8,
    },
    Teleport {
        index: usize,
    },
    SpawnTeleport {
        index: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShapeType {
    Box,
    Sphere,
    Cylinder,
}

#[derive(Clone, Copy)]
pub struct Shape {
    id: Uuid,
    pub shape_type: ShapeType,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub size: [f32; 3],
    // pub color: [f32; 4],
    pub color: Hsva,
}

impl Shape {
    pub fn new() -> Self {
        Shape {
            id: Uuid::new_v4(),
            shape_type: ShapeType::Box,
            position: [0., 0., 0.],
            rotation: [0., 0., 0.],
            size: [1., 1., 1.],
            // color: [1., 1., 1., 1.],
            color: Hsva::from_rgb([1., 1., 1.]),
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }
}

pub struct UIState {
    // pub events: VecDeque<UIEvent>,
    pub file_path_rx: Option<RX>,
    tab: Tab,
    pub modifier: u8,
    delete_mode: bool,
    renaming_collection: Option<Uuid>,
    renaming_name: String,
    pub teleports: [ Option<Teleport>; 2 ],
    pub hide_checkpoints: bool,

    pub custom_shapes: Vec<(Shape, bool)>,
}

impl UIState {
    pub fn init() -> UIState {
        let ui_state = UIState {
            // events: VecDeque::new(),
            file_path_rx: None,
            tab: Tab::Comparison,
            modifier: 0,
            delete_mode: false,
            renaming_collection: None,
            renaming_name: "".to_string(),
            teleports: [None, None],
            hide_checkpoints: false,
            custom_shapes: Vec::new(),
        };

        ui_state
    }
}

pub fn check_input(input: &egui::RawInput) {
    let config = CONFIG_STATE.lock().unwrap();

    let direct_mode = config.direct_mode;
    let start_keybind = config.start_keybind;
    let stop_keybind = config.stop_keybind;
    let reset_keybind = config.reset_keybind;
    let clear_keybind = config.clear_keybind;
    let teleport_keybinds = config.teleport_keybinds;
    let spawn_teleport_keybinds = config.spawn_teleport_keybinds;

    drop(config);

    let mut new_events : VecDeque<UIEvent> = VecDeque::new();

    let mut modifier = 0;

    if input.modifiers.shift {
        modifier = 1;
    }

    if input.modifiers.ctrl {
        modifier = 2;
    }

    for e in &input.events {
        if start_keybind.compare_to_event(e) {
            if direct_mode {
                new_events.push_back(UIEvent::StartRecording);
            } else {
                new_events.push_back(UIEvent::SpawnTrigger {
                    index: 0,
                    position: gamedata::get_player_position(),
                    rotation: gamedata::get_player_rotation(),
                });
            }
        }

        if stop_keybind.compare_to_event(e) {
            if direct_mode {
                new_events.push_back(UIEvent::StopRecording);
            } else {
                new_events.push_back(UIEvent::SpawnTrigger {
                    index: 1,
                    position: gamedata::get_player_position(),
                    rotation: gamedata::get_player_rotation(),
                });
            }
        }

        if reset_keybind.compare_to_event(e) {
            new_events.push_back(UIEvent::ResetRecording);
        }

        if clear_keybind.compare_to_event(e) {
            new_events.push_back(UIEvent::ClearTriggers);
        }

        if teleport_keybinds[0].compare_to_event(e) {
            new_events.push_back(UIEvent::Teleport { index: 0 });
        }

        if teleport_keybinds[1].compare_to_event(e) {
            new_events.push_back(UIEvent::Teleport { index: 1 });
        }

        if spawn_teleport_keybinds[0].compare_to_event(e) {
            new_events.push_back(UIEvent::SpawnTeleport { index: 0 });
        }

        if spawn_teleport_keybinds[1].compare_to_event(e) {
            new_events.push_back(UIEvent::SpawnTeleport { index: 1 });
        }

        // if config.spawn_checkpoint_keybind.compare_to_event(e) {
        //     egui.new_events.push_back(UIEvent::SpawnTrigger {
        //         index: 2,
        //         position: gamedata::get_player_position(),
        //         rotation: gamedata::get_player_rotation(),
        //         size: config.trigger_size[1],
        //     });
        // }
    }

    let mut ui_state = UISTATE.lock().unwrap();

    ui_state.modifier = modifier;

    drop(ui_state);

    let mut events = EVENTS.lock().unwrap();

    events.append(&mut new_events);
}

pub fn draw_ui(ui: &mut egui::Ui) {
    let config = CONFIG_STATE.lock().unwrap();

    let accent_colors = config.accent_colors;
    let shapes_enabled = config.shapes_enabled;

    drop(config);

    let ui_state = UISTATE.lock().unwrap();

    let mut tab = ui_state.tab;

    drop(ui_state);

    ui.visuals_mut().selection.bg_fill = accent_colors[0];

    ui.spacing_mut().item_spacing = egui::vec2(15.0, 3.0);
    // ui.spacing_mut().window_margin = egui::Margin::same(50.0);

    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
        ui.selectable_value(&mut tab, Tab::Comparison, egui::RichText::new("Comparison").strong());
        ui.selectable_value(&mut tab, Tab::Triggers, egui::RichText::new("Triggers").strong());
        if shapes_enabled {
            ui.selectable_value(&mut tab, Tab::CustomShapes, egui::RichText::new("Custom Shapes").strong());
        }
        ui.selectable_value(&mut tab, Tab::Config, egui::RichText::new("Config").strong());
        ui.selectable_value(&mut tab, Tab::Credits, egui::RichText::new("Credits").strong());
    });

    UISTATE.lock().unwrap().tab = tab;

    // ui.separator();

    ui.spacing_mut().item_spacing = egui::vec2(10.0, 3.0);

    draw_comparison_tab(ui);

    ui.scope(|ui| {
        ui.set_enabled(PATHLOG.lock().unwrap().is_empty());
        draw_triggers_tab(ui);
    });

    draw_custom_shapes_tab(ui);
    draw_config_tab(ui);
    draw_credits_tab(ui);
}

pub fn draw_timer(ui: &mut egui::Ui) {
    let time = PATHLOG.lock().unwrap().time();
    let timer_size = CONFIG_STATE.lock().unwrap().timer_size;

    ui.add(egui::Label::new(
        egui::RichText::new(format!("{:02}:{:02}.{:03}", time / 60000, (time % 60000) / 1000, (time % 1000)))
        .size(timer_size)
    ).selectable(false));
}

fn draw_comparison_tab(ui: &mut egui::Ui) {
    if UISTATE.lock().unwrap().tab != Tab::Comparison { return; }

    let pathlog = PATHLOG.lock().unwrap();

    let active_collection = pathlog.active_collection;
    let path_collections_len = pathlog.collections().len();

    drop(pathlog);

    let config = CONFIG_STATE.lock().unwrap();

    let accent_colors = config.accent_colors;

    drop(config);

    let ui_state = UISTATE.lock().unwrap();

    let mut renaming_collection = ui_state.renaming_collection;
    let mut renaming_name = ui_state.renaming_name.clone();
    let mut delete_mode = ui_state.delete_mode;

    drop(ui_state);

    let mut new_events : VecDeque<UIEvent> = VecDeque::new();
    let mut mute_toggles : Vec<Uuid> = Vec::new();
    let mut solo_toggles : Vec<Uuid> = Vec::new();
    let mut to_clear : Vec<Uuid> = Vec::new();

    ui.separator();

    if ui.interact_bg(egui::Sense::click()).clicked() {
        renaming_collection = None;
    }

    for i in 0..path_collections_len {
        let pathlog = PATHLOG.lock().unwrap();

        let collection_name = pathlog.collections()[i].name.clone();
        let collection_id = pathlog.collections()[i].id();
        let collection_len = pathlog.collections()[i].paths().len();
        let mute = *pathlog.mute_collections.get(&collection_id).unwrap();
        let solo = *pathlog.solo_collections.get(&collection_id).unwrap();
        let high_pass = pathlog.filters.get(&collection_id).cloned();

        drop(pathlog);

        egui::Grid::new(collection_id.to_string() + "buttons")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            // if ui.interact_bg(egui::Sense::click()).clicked() {
            //     state.ui_state.renaming_collection = None;
            // }
            let original_hovered_weak_bg_fill = ui.visuals_mut().widgets.hovered.weak_bg_fill;
            let original_inactive_weak_bg_fill = ui.visuals_mut().widgets.inactive.weak_bg_fill;

            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                let mut arm_button_text = egui::RichText::new("\u{2B55}");
                if active_collection == Some(collection_id) {
                    ui.visuals_mut().widgets.hovered.weak_bg_fill = accent_colors[1].gamma_multiply(1.2);
                    ui.visuals_mut().widgets.inactive.weak_bg_fill = accent_colors[1];
                    arm_button_text = arm_button_text.strong();
                }

                if ui.add(
                    egui::Button::new(arm_button_text)
                        .min_size(egui::vec2(19.0, 19.0))
                        .rounding(egui::Rounding::same(10.0))
                    ).clicked() {
                    new_events.push_back(UIEvent::ToggleActive { id: collection_id });
                }

                ui.visuals_mut().widgets.hovered.weak_bg_fill = original_hovered_weak_bg_fill;
                ui.visuals_mut().widgets.inactive.weak_bg_fill = original_inactive_weak_bg_fill;

                if renaming_collection == Some(collection_id) {
                    let response = ui.add(egui::TextEdit::singleline(&mut renaming_name).desired_width(200.0));
                    if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let new_name = renaming_name.clone();
                        new_events.push_back(UIEvent::RenameCollection { id: collection_id, new_name });
                        renaming_collection = None;
                    }
                }
                else if ui.label(collection_name.clone()).clicked() {
                    renaming_collection = Some(collection_id);
                    renaming_name = collection_name.clone();
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if delete_mode {
                    if ui.add(egui::Button::new("\u{1F5D9}").min_size(egui::vec2(19.0, 19.0))).clicked() {
                        new_events.push_back(UIEvent::DeleteCollection { id: collection_id });
                    }
                }

                let mut solo_button_text = egui::RichText::new("\u{1F1F8}");

                if solo {
                    ui.visuals_mut().widgets.hovered.weak_bg_fill = accent_colors[0].gamma_multiply(1.2);
                    ui.visuals_mut().widgets.inactive.weak_bg_fill = accent_colors[0];
                    solo_button_text = solo_button_text.strong();
                }

                if ui.add(egui::Button::new(solo_button_text).min_size(egui::vec2(19.0, 19.0))).clicked() {
                    // *state.ui_state.solo_collections.get_mut(&collection_id).unwrap() ^= true;
                    solo_toggles.push(collection_id);
                }

                ui.visuals_mut().widgets.hovered.weak_bg_fill = original_hovered_weak_bg_fill;
                ui.visuals_mut().widgets.inactive.weak_bg_fill = original_inactive_weak_bg_fill;

                let mut mute_button_text = egui::RichText::new("\u{1F1F2}");
                if mute {
                    ui.visuals_mut().widgets.hovered.weak_bg_fill = accent_colors[0].gamma_multiply(1.2);
                    ui.visuals_mut().widgets.inactive.weak_bg_fill = accent_colors[0];
                    mute_button_text = mute_button_text.strong();
                }

                if ui.add(egui::Button::new(mute_button_text).min_size(egui::vec2(19.0, 19.0))).clicked() {
                    mute_toggles.push(collection_id);
                }

                ui.visuals_mut().widgets.hovered.weak_bg_fill = original_hovered_weak_bg_fill;
                ui.visuals_mut().widgets.inactive.weak_bg_fill = original_inactive_weak_bg_fill;

                let mut mute_button_text = egui::RichText::new("\u{2B06}");
                if let Some(HighPassFilter::Gold) = high_pass {
                    ui.visuals_mut().widgets.hovered.weak_bg_fill = accent_colors[0].gamma_multiply(1.2);
                    ui.visuals_mut().widgets.inactive.weak_bg_fill = accent_colors[0];
                    mute_button_text = mute_button_text.strong();
                }

                if ui.add(egui::Button::new(mute_button_text).min_size(egui::vec2(19.0, 19.0))).clicked() {
                    new_events.push_back(UIEvent::ToggleGoldFilter { collection_id });
                }

                ui.visuals_mut().widgets.hovered.weak_bg_fill = original_hovered_weak_bg_fill;
                ui.visuals_mut().widgets.inactive.weak_bg_fill = original_inactive_weak_bg_fill;
            });
            ui.end_row();
        });

        egui::CollapsingHeader::new("").id_source(collection_id.to_string() + "collapsing")
            .show(ui, |ui| {
                egui::Grid::new(collection_id.to_string() + "paths")
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(false)
                    // .with_row_color(|i, style| {
                    //     // this is not pretty but I'm just glad it works for now
                    //     // nah actually fuck this rn
                    //     unsafe {
                    //         let p_log = &GLOBAL_STATE.as_mut().unwrap().state.pathlog;
                    //         if let Some(c_id) = p_log.active_collection {
                    //             let coll = &p_log.path_collections[p_log.path_collections.iter().position(|c| c.id() == c_id).unwrap()];
                    //             if i < coll.paths().len() && coll.paths()[i].id() == p_log.latest_path {
                    //                 Some(egui::Color32::from_gray(42))
                    //                 // Some(style.visuals.faint_bg_color)
                    //             }
                    //             else { None }
                    //         }
                    //         else { None }
                    //     }
                    // })
                    .show(ui, |ui| {
                        if ui.interact_bg(egui::Sense::click()).clicked() {
                            // state.ui_state.selected_paths.get_mut(&collection_id).unwrap().clear();
                            renaming_collection = None;
                            to_clear.push(collection_id);
                        }

                        // for path in state.pathlog.path_collections[i].paths() {
                        for p in 0..collection_len {
                            draw_path(ui, p, i);
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
                    new_events.push_back(UIEvent::SaveComparison);
                }
                if ui.add(egui::Button::new("Load").min_size(egui::vec2(19.0, 19.0))).clicked() {
                    new_events.push_back(UIEvent::LoadComparison);
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.add(egui::Button::new("\u{2796}").min_size(egui::vec2(19.0, 19.0))).clicked() {
                    delete_mode ^= true;
                }
                if ui.add(egui::Button::new("\u{2795}").min_size(egui::vec2(19.0, 19.0))).clicked() {
                    new_events.push_back(UIEvent::CreateCollection);
                }
            });
            ui.end_row();
        });

    let mut pathlog = PATHLOG.lock().unwrap();

    for collection_id in to_clear {
        pathlog.selected_paths.get_mut(&collection_id).unwrap().clear();
    }

    drop(pathlog);

    let mut ui_state = UISTATE.lock().unwrap();

    ui_state.renaming_collection = renaming_collection;
    ui_state.renaming_name = renaming_name;
    ui_state.delete_mode = delete_mode;

    drop(ui_state);

    let mut events = EVENTS.lock().unwrap();

    for collection_id in solo_toggles {
        events.push_back(UIEvent::ToggleSolo { id: collection_id });
    }

    for collection_id in mute_toggles {
        events.push_back(UIEvent::ToggleMute { id: collection_id });
    }

    events.append(&mut new_events);
}

fn draw_path(ui: &mut egui::Ui, path: usize, collection: usize) {
    let pathlog = PATHLOG.lock().unwrap();

    let path_id = pathlog.collections()[collection].paths()[path];
    let path_time = pathlog.path(&path_id).unwrap().time();
    let collection_id = pathlog.collections()[collection].id();
    let latest_path = pathlog.latest_path;
    let selected = pathlog.selected_paths.get(&collection_id).unwrap().clone();
    let mute = *pathlog.mute_paths.get(&path_id).unwrap();
    let solo = *pathlog.solo_paths.get(&path_id).unwrap();
    let high_pass = pathlog.filters.get(&collection_id).cloned();

    drop(pathlog);

    let config = CONFIG_STATE.lock().unwrap();

    let accent_colors = config.accent_colors;
    let select_color = config.select_color;

    drop(config);

    let ui_state = UISTATE.lock().unwrap();

    let mods = ui_state.modifier;
    let delete_mode = ui_state.delete_mode;

    drop(ui_state);

    let mut new_events : VecDeque<UIEvent> = VecDeque::new();
    let mut mute_toggle = false;
    let mut solo_toggle = false;
    let mut delete = false;

    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {

        let original_hovered_weak_bg_fill = ui.visuals_mut().widgets.hovered.weak_bg_fill;
        let original_inactive_weak_bg_fill = ui.visuals_mut().widgets.inactive.weak_bg_fill;

        let mut mute_button_text = egui::RichText::new("\u{1F1F2}");

        if mute {
            ui.visuals_mut().widgets.hovered.weak_bg_fill = accent_colors[0].gamma_multiply(1.2);
            ui.visuals_mut().widgets.inactive.weak_bg_fill = accent_colors[0];
            mute_button_text = mute_button_text.strong();
        }

        if ui.add(egui::Button::new(mute_button_text).min_size(egui::vec2(19.0, 19.0))).clicked() {
            mute_toggle = true;
        }

        ui.visuals_mut().widgets.hovered.weak_bg_fill = original_hovered_weak_bg_fill;
        ui.visuals_mut().widgets.inactive.weak_bg_fill = original_inactive_weak_bg_fill;

        let mut solo_button_text = egui::RichText::new("\u{1F1F8}");

        if solo {
            ui.visuals_mut().widgets.hovered.weak_bg_fill = accent_colors[0].gamma_multiply(1.2);
            ui.visuals_mut().widgets.inactive.weak_bg_fill = accent_colors[0];
            solo_button_text = solo_button_text.strong();
        }

        if ui.add(egui::Button::new(solo_button_text).min_size(egui::vec2(19.0, 19.0))).clicked() {
            solo_toggle = true;
        }

        ui.visuals_mut().widgets.hovered.weak_bg_fill = ui.visuals_mut().window_fill;
        ui.visuals_mut().widgets.inactive.weak_bg_fill = ui.visuals_mut().window_fill;

        // TODO: why tf do these not disable the dark line around the buttons??
        // let original_bg_stroke_color = ui.visuals_mut().widgets.inactive.bg_stroke.color;
        // ui.visuals_mut().widgets.inactive.bg_stroke.color = ui.visuals_mut().window_fill;
        // let original_bg_stroke = ui.visuals_mut().widgets.inactive.bg_stroke;
        // ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::NONE;

        // let mods = state.ui_state.modifier;
        // let selected = state.ui_state.selected_paths.get_mut(&collection_id).unwrap();
        if selected.contains(&path_id) {
            ui.visuals_mut().override_text_color = Some(select_color.as_color32());
        }
        if latest_path == path_id {
            ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::from_gray(42);
        }

        // let time_response = ui.add(egui::Label::new(format!("{:02}:{:02}.{:03}", time / 60000, (time % 60000) / 1000, (time % 1000))).selectable(true));

        let time_text = egui::RichText::new(format!("{:02}:{:02}.{:03}", path_time / 60000, (path_time % 60000) / 1000, (path_time % 1000)));

        let time_response = ui.add(egui::Button::new(time_text).min_size(egui::vec2(80.0, 19.0)));

        if time_response.clicked() {
            new_events.push_back(UIEvent::SelectPath { path_id, collection_id, modifier: mods });
        }
        if time_response.secondary_clicked() {
            new_events.push_back(UIEvent::SetPathFilter { collection_id, path_id });
        }

        ui.visuals_mut().widgets.hovered.weak_bg_fill = original_hovered_weak_bg_fill;
        ui.visuals_mut().widgets.inactive.weak_bg_fill = original_inactive_weak_bg_fill;
        // ui.visuals_mut().widgets.inactive.bg_stroke.color = original_bg_stroke_color;
        // ui.visuals_mut().widgets.inactive.bg_stroke = original_bg_stroke;

        ui.visuals_mut().override_text_color = None;

        // TODO: maybe implement PartialEq for HighPassFilter
        if let Some(filter) = high_pass {
            if let HighPassFilter::Path{ id } = filter {
                if id == path_id {
                    ui.label("\u{2B06}");
                }
            }
        }
    });

    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
        if delete_mode {
            if ui.add(egui::Button::new("\u{1F5D9}").min_size(egui::vec2(19.0, 19.0))).clicked() {
                delete = true;
            }
        }
    });

    ui.end_row();

    let mut events = EVENTS.lock().unwrap();

    if mute_toggle {
        events.push_back(UIEvent::ToggleMute { id: path_id });
    }

    if solo_toggle {
        events.push_back(UIEvent::ToggleSolo { id: path_id });
    }

    drop(events);

    if delete {
        PATHLOG.lock().unwrap().selected_paths.get_mut(&collection_id).unwrap().clear();
        EVENTS.lock().unwrap().push_back(UIEvent::DeletePath { path_id });
    }

    UISTATE.lock().unwrap().delete_mode = delete_mode;
    EVENTS.lock().unwrap().append(&mut new_events);
}

fn draw_triggers_tab(ui: &mut egui::Ui) {
    if UISTATE.lock().unwrap().tab != Tab::Triggers { return; }

    let pathlog = PATHLOG.lock().unwrap();

    let checkpoint_triggers_len = pathlog.checkpoint_triggers.len();

    drop(pathlog);

    let ui_state = UISTATE.lock().unwrap();

    let mut delete_mode = ui_state.delete_mode;

    drop(ui_state);

    ui.separator();

    let mut delete_list: Vec<Uuid> = Vec::new();

    for t in 0..(checkpoint_triggers_len + 2) {
        draw_trigger(ui, t, &mut delete_list);
    }

    egui::Grid::new("util")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |_ui| {
                // let original_hovered_weak_bg_fill = ui.visuals_mut().widgets.hovered.weak_bg_fill;
                // let original_inactive_weak_bg_fill = ui.visuals_mut().widgets.inactive.weak_bg_fill;

                // let mut mute_button_text = egui::RichText::new("\u{1F1F2}");

                // if state.ui_state.hide_checkpoints {
                //     ui.visuals_mut().widgets.hovered.weak_bg_fill = config.accent_colors[0].gamma_multiply(1.2);
                //     ui.visuals_mut().widgets.inactive.weak_bg_fill = config.accent_colors[0];
                //     mute_button_text = mute_button_text.strong();
                // }

                // if ui.add(egui::Button::new(mute_button_text).min_size(egui::vec2(19.0, 19.0))).clicked() {
                //     state.ui_state.hide_checkpoints ^= true;
                // }

                // ui.visuals_mut().widgets.hovered.weak_bg_fill = original_hovered_weak_bg_fill;
                // ui.visuals_mut().widgets.inactive.weak_bg_fill = original_inactive_weak_bg_fill;
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.add(egui::Button::new("\u{2796}").min_size(egui::vec2(19.0, 19.0))).clicked() {
                    delete_mode ^= true;
                }
                // if ui.add(egui::Button::new("\u{2795}").min_size(egui::vec2(19.0, 19.0))).clicked() {
                //     state.pathlog.checkpoint_triggers.push(BoxCollider::new(pos, rotation, size));
                // }
            });
            ui.end_row();
        });

    let mut pathlog = PATHLOG.lock().unwrap();

    for id in delete_list {
        let pos = pathlog.checkpoint_triggers.iter().position(|t| t.id() == id);
        if let Some(i) = pos { pathlog.checkpoint_triggers.remove(i); }
    }

    drop(pathlog);

    let mut ui_state = UISTATE.lock().unwrap();

    ui_state.delete_mode = delete_mode;
}

fn draw_trigger(ui: &mut egui::Ui, trigger_index: usize, delete_list: &mut Vec<Uuid>) {
    let pathlog = PATHLOG.lock().unwrap();

    let mut trigger = match trigger_index {
        0 | 1 => {
            let t = pathlog.main_triggers[trigger_index];
            if t.is_none() { return; }
            t.unwrap()
        },
        _ => pathlog.checkpoint_triggers[trigger_index - 2],
    };

    drop(pathlog);

    let ui_state = UISTATE.lock().unwrap();

    let delete_mode = ui_state.delete_mode;

    drop(ui_state);

    egui::Grid::new(trigger.id().to_string() + "buttons")
    .num_columns(2)
    .spacing([40.0, 4.0])
    .striped(true)
    .show(ui, |ui| {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
            // egui::ComboBox::new(shape_id.to_string() + "drop_down", "")
            // .selected_text(format!("{:?}", shape.0.shape_type))
            // .show_ui(ui, |ui| {
            //     ui.selectable_value(&mut shape.0.shape_type, ShapeType::Box, "Box");
            //     ui.selectable_value(&mut shape.0.shape_type, ShapeType::Sphere, "Sphere");
            //     ui.selectable_value(&mut shape.0.shape_type, ShapeType::Cylinder, "Cylinder");
            // });

            ui.label("Checkpoint");
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            if delete_mode {
                if ui.add(egui::Button::new("\u{1F5D9}").min_size(egui::vec2(19.0, 19.0))).clicked() {
                    delete_list.push(trigger.id());
                }
            }
            // ui.color_edit_button_hsva(&mut shape.color);
        });
        ui.end_row();
    });

    egui::CollapsingHeader::new("").id_source(trigger.id().to_string() + "collapsing")
        .show(ui, |ui| {
            egui::Grid::new(trigger.id().to_string() + "data")
            .num_columns(2)
            .spacing([10.0, 4.0])
            .striped(false)
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                    ui.label("Position");
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    ui.add(egui::DragValue::new(&mut trigger.position[2]).speed(0.1));
                    ui.add(egui::DragValue::new(&mut trigger.position[1]).speed(0.1));
                    ui.add(egui::DragValue::new(&mut trigger.position[0]).speed(0.1));
                });
                ui.end_row();

                let mut rot = trigger.rotation();

                ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                    ui.label("Rotation");
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    if ui.add(egui::DragValue::new(&mut rot[2]).speed(0.01).clamp_range(-PI..=PI)).changed() { trigger.set_rotation(rot); };
                    if ui.add(egui::DragValue::new(&mut rot[1]).speed(0.01).clamp_range(-PI..=PI)).changed() { trigger.set_rotation(rot); };
                    if ui.add(egui::DragValue::new(&mut rot[0]).speed(0.01).clamp_range(-PI..=PI)).changed() { trigger.set_rotation(rot); };
                });
                ui.end_row();

                ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                    ui.label("Size");
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    ui.add(egui::DragValue::new(&mut trigger.size[2]).speed(0.1).clamp_range(0.0..=42069.0));
                    ui.add(egui::DragValue::new(&mut trigger.size[1]).speed(0.1).clamp_range(0.0..=42069.0));
                    ui.add(egui::DragValue::new(&mut trigger.size[0]).speed(0.1).clamp_range(0.0..=42069.0));
                });
                ui.end_row();
            });
        });

    ui.separator();

    // ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
    //     if state.ui_state.delete_mode {
    //         if ui.add(egui::Button::new("\u{1F5D9}").min_size(egui::vec2(19.0, 19.0))).clicked() {
    //             delete_list.push(trigger.id());
    //         }
    //     }
    // });

    ui.end_row();

    let mut pathlog = PATHLOG.lock().unwrap();

    match trigger_index {
        0 | 1 => pathlog.main_triggers[trigger_index] = Some(trigger),
        _ => pathlog.checkpoint_triggers[trigger_index - 2] = trigger,
    };
}

fn draw_config_tab(ui: &mut egui::Ui) {
    if UISTATE.lock().unwrap().tab != Tab::Config { return; }

    let config = CONFIG_STATE.lock().unwrap();

    let mut autosave = config.autosave;
    let mut autoreset = config.autoreset;
    let mut zoom = config.zoom;
    let mut trigger_sizes = config.trigger_sizes;

    let direct_mode = config.direct_mode;
    let mut start_keybind = config.start_keybind;
    let mut stop_keybind = config.stop_keybind;
    let mut reset_keybind = config.reset_keybind;
    let mut clear_keybind = config.clear_keybind;
    let mut teleport_keybinds = config.teleport_keybinds;
    let mut spawn_teleport_keybinds = config.spawn_teleport_keybinds;

    let mut timer_size = config.timer_size;
    // let mut timer_position = config.timer_position;
	let mut trigger_colors = config.trigger_colors;
	// let mut checkpoint_color = config.checkpoint_color;
    let mut fast_color = config.fast_color;
    let mut slow_color = config.slow_color;
    let mut gold_color = config.gold_color;
    let mut select_color = config.select_color;
    let mut accent_colors = config.accent_colors;

    // pub custom_shapes: bool,

    drop(config);

    let mut events : VecDeque<UIEvent> = VecDeque::new();

    ui.separator();

    ui.add_space(5.0);
    egui::Grid::new("toggles_grid")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            // ui.label("Direct Recording Mode");
            // ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            //     if toggle_switch(ui, &mut config.direct_mode).clicked() {
            //         state.ui_state.events.push_back(UIEvent::ChangeDirectMode { new: config.direct_mode });
            //     }
            // });
            // ui.end_row();

            ui.label("Comparison Autosave");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if toggle_switch(ui, &mut autosave).clicked() {
                    let new = autosave;
                    events.push_back(UIEvent::ChangeAutosave { new });
                }
            });
            ui.end_row();

            ui.label("Reset Recording on Start");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let new = autoreset;
                if toggle_switch(ui, &mut autoreset).clicked() {
                    events.push_back(UIEvent::ChangeAutoReset { new });
                }
            });
            ui.end_row();

            ui.label("UI Scale");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(egui::DragValue::new(&mut zoom).speed(0.1).clamp_range(0.3..=3.0));
            });
            ui.end_row();

            ui.label("Start Trigger Size");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(egui::DragValue::new(&mut trigger_sizes[0][2]).speed(0.1).clamp_range(0.1..=10.0));
                ui.add(egui::DragValue::new(&mut trigger_sizes[0][1]).speed(0.1).clamp_range(0.1..=10.0));
                ui.add(egui::DragValue::new(&mut trigger_sizes[0][0]).speed(0.1).clamp_range(0.1..=10.0));
            });
            ui.end_row();

            ui.label("End Trigger Size");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(egui::DragValue::new(&mut trigger_sizes[1][2]).speed(0.1).clamp_range(0.1..=10.0));
                ui.add(egui::DragValue::new(&mut trigger_sizes[1][1]).speed(0.1).clamp_range(0.1..=10.0));
                ui.add(egui::DragValue::new(&mut trigger_sizes[1][0]).speed(0.1).clamp_range(0.1..=10.0));
            });
            ui.end_row();

            ui.label("Timer Size");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(egui::DragValue::new(&mut timer_size).speed(0.5).clamp_range(6.9..=69.0));
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
            if direct_mode {
                ui.label("Start Recording");
            }
            else {
                ui.label("Spawn Start Trigger");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(Keybind::new(&mut start_keybind, "start_keybind"));
            });
            ui.end_row();

            if direct_mode {
                ui.label("Stop Recording");
            }
            else {
                ui.label("Spawn End Trigger");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(Keybind::new(&mut stop_keybind, "stop_keybind"));
            });
            ui.end_row();

            ui.label("Reset Recording");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(Keybind::new(&mut reset_keybind, "reset_keybind"));
            });
            ui.end_row();

            ui.label("Delete Triggers");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(Keybind::new(&mut clear_keybind, "clear_keybind"));
            });
            ui.end_row();

            ui.label("Teleport to Location 1");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(Keybind::new(&mut teleport_keybinds[0], "teleport_1_keybind"));
            });
            ui.end_row();

            ui.label("Teleport to Location 2");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(Keybind::new(&mut teleport_keybinds[1], "teleport_2_keybind"));
            });
            ui.end_row();

            ui.label("Set Teleport Location 1");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(Keybind::new(&mut spawn_teleport_keybinds[0], "spawn_teleport_1_keybind"));
            });
            ui.end_row();

            ui.label("Set Teleport Location 2");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(Keybind::new(&mut spawn_teleport_keybinds[1], "spawn_teleport_2_keybind"));
            });
            ui.end_row();

            // ui.label("Spawn Checkpoint");
            // ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            //     ui.add(Keybind::new(&mut config.spawn_checkpoint_keybind, "spawn_checkpoint_keybind"));
            // });
            // ui.end_row();
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
                let mut tmp_color = trigger_colors[0].as_hsva();
                if ui.color_edit_button_hsva(&mut tmp_color).changed() {
                    trigger_colors[0] = tmp_color.to_rgba_premultiplied();
                }
            });
            ui.end_row();

            ui.label("End Trigger");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut tmp_color = trigger_colors[1].as_hsva();
                if ui.color_edit_button_hsva(&mut tmp_color).changed() {
                    trigger_colors[1] = tmp_color.to_rgba_premultiplied();
                }
            });
            ui.end_row();

            ui.label("Path Gradient: Fast");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut tmp_color = fast_color.as_hsva();
                if ui.color_edit_button_hsva(&mut tmp_color).changed() {
                    fast_color = tmp_color.to_rgba_premultiplied();
                }
            });
            ui.end_row();

            ui.label("Path Gradient: Slow");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut tmp_color = slow_color.as_hsva();
                if ui.color_edit_button_hsva(&mut tmp_color).changed() {
                    slow_color = tmp_color.to_rgba_premultiplied();
                }
            });
            ui.end_row();

            ui.label("Fastest Path");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut tmp_color = gold_color.as_hsva();
                if ui.color_edit_button_hsva(&mut tmp_color).changed() {
                    gold_color = tmp_color.to_rgba_premultiplied();
                }
            });
            ui.end_row();

            ui.label("Selected Path");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut tmp_color = select_color.as_hsva();
                if ui.color_edit_button_hsva(&mut tmp_color).changed() {
                    select_color = tmp_color.to_rgba_premultiplied();
                }
            });
            ui.end_row();

            ui.end_row();
            ui.label(egui::RichText::new("UI").size(15.0));
            ui.end_row();

            ui.label("Accent 0");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut tmp_color = accent_colors[0].as_hsva();
                if ui.color_edit_button_hsva(&mut tmp_color).changed() {
                    accent_colors[0] = tmp_color.as_color32();
                }
            });
            ui.end_row();

            ui.label("Accent 1");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut tmp_color = accent_colors[1].as_hsva();
                if ui.color_edit_button_hsva(&mut tmp_color).changed() {
                   accent_colors[1] = tmp_color.as_color32();
                }
            });
            ui.end_row();
        });

    ui.separator();

    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
        if ui.add(egui::Button::new("Save")).clicked() {
            // state.config.write("celestial.ini".to_string());
            events.push_back(UIEvent::SaveConfig);
        }
        if ui.add(egui::Button::new("Load")).clicked() {
            // if let Err(e) = state.config.read("celestial.ini".to_string()) {
            //     println!("{e}"); // ini error doesn't implement tracing::Value. maybe change this
            // }
            events.push_back(UIEvent::LoadConfig);
        }
    });

    let mut config = CONFIG_STATE.lock().unwrap();

    config.autosave = autosave;
    config.autoreset = autoreset;

    config.zoom = zoom;
    config.trigger_sizes = trigger_sizes;

    config.start_keybind = start_keybind;
    config.stop_keybind = stop_keybind;
    config.reset_keybind = reset_keybind;
    config.clear_keybind = clear_keybind;
    config.teleport_keybinds = teleport_keybinds;
    config.spawn_teleport_keybinds = spawn_teleport_keybinds;

    config.timer_size = timer_size;
    // config.timer_position = timer_position;
	config.trigger_colors = trigger_colors;
	// config.checkpoint_color = checkpoint_color;
    config.fast_color = fast_color;
    config.slow_color = slow_color;
    config.gold_color = gold_color;
    config.select_color = select_color;
    config.accent_colors = accent_colors;
}

// fn draw_credits_tab(ui: &mut egui::Ui, state.ui_state: &mut UIState) {
fn draw_credits_tab(ui: &mut egui::Ui) {
    if UISTATE.lock().unwrap().tab != Tab::Credits { return; }

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

            ui.hyperlink_to("Vluurie", "https://github.com/Vluurie");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("Programming Support"); });
            ui.end_row();

            ui.hyperlink_to("Aloyark", "https://www.twitch.tv/aloyarkk");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("Testing & Feedback"); });
            ui.end_row();

            ui.hyperlink_to("Icarus", "https://www.twitch.tv/icarus_042");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("Testing & Feedback"); });
            ui.end_row();

            ui.hyperlink_to("Percy", "https://www.twitch.tv/percyz01");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("Testing & Feedback"); });
            ui.end_row();
        });
}

fn draw_custom_shapes_tab(ui: &mut egui::Ui) {
    if UISTATE.lock().unwrap().tab != Tab::CustomShapes { return; }

    let config = CONFIG_STATE.lock().unwrap();

    let accent_colors = config.accent_colors;

    drop(config);

    let ui_state = UISTATE.lock().unwrap();

    let mut custom_shapes = ui_state.custom_shapes.clone();
    let mut delete_mode = ui_state.delete_mode;

    drop(ui_state);

    ui.separator();

    let mut delete_list: Vec<Uuid> = Vec::new();

    // for shape in &mut custom_shapes {
    for s in 0..custom_shapes.len() {

        let shape_id = custom_shapes[s].0.id();
        // let muted = &mut custom_shapes[s].1;

        egui::Grid::new(shape_id.to_string() + "buttons")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            // let shape = &mut custom_shapes[s];

            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                // let shape = &mut custom_shapes[s];

                let original_hovered_weak_bg_fill = ui.visuals_mut().widgets.hovered.weak_bg_fill;
                let original_inactive_weak_bg_fill = ui.visuals_mut().widgets.inactive.weak_bg_fill;

                let mut mute_button_text = egui::RichText::new("\u{1F1F2}");

                if custom_shapes[s].1 {
                    ui.visuals_mut().widgets.hovered.weak_bg_fill = accent_colors[0].gamma_multiply(1.2);
                    ui.visuals_mut().widgets.inactive.weak_bg_fill = accent_colors[0];
                    mute_button_text = mute_button_text.strong();
                }

                if ui.add(egui::Button::new(mute_button_text).min_size(egui::vec2(19.0, 19.0))).clicked() {
                    custom_shapes[s].1 ^= true;
                }

                ui.visuals_mut().widgets.hovered.weak_bg_fill = original_hovered_weak_bg_fill;
                ui.visuals_mut().widgets.inactive.weak_bg_fill = original_inactive_weak_bg_fill;

                let shape = &mut custom_shapes[s];

                egui::ComboBox::new(shape_id.to_string() + "drop_down", "")
                .selected_text(format!("{:?}", shape.0.shape_type))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut shape.0.shape_type, ShapeType::Box, "Box");
                    ui.selectable_value(&mut shape.0.shape_type, ShapeType::Sphere, "Sphere");
                    ui.selectable_value(&mut shape.0.shape_type, ShapeType::Cylinder, "Cylinder");
                });
            });

            // ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            //     if state.ui_state.delete_mode {
            //         if ui.add(egui::Button::new("\u{1F5D9}").min_size(egui::vec2(19.0, 19.0))).clicked() {
            //             delete_list.push(shape_id);
            //         }
            //     }
            // });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if delete_mode {
                    if ui.add(egui::Button::new("\u{1F5D9}").min_size(egui::vec2(19.0, 19.0))).clicked() {
                        delete_list.push(shape_id);
                    }
                }

                ui.color_edit_button_hsva(&mut custom_shapes[s].0.color);
            });
            ui.end_row();
        });

        egui::CollapsingHeader::new("").id_source(shape_id.to_string() + "collapsing")
            .show(ui, |ui| {
                egui::Grid::new(shape_id.to_string() + "paths")
                .num_columns(2)
                .spacing([10.0, 4.0])
                .striped(false)
                .show(ui, |ui| {
                    let shape = &mut custom_shapes[s];

                    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                        ui.label("Position");
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        ui.add(egui::DragValue::new(&mut shape.0.position[2]).speed(0.1));
                        ui.add(egui::DragValue::new(&mut shape.0.position[1]).speed(0.1));
                        ui.add(egui::DragValue::new(&mut shape.0.position[0]).speed(0.1));
                    });
                    ui.end_row();

                    match shape.0.shape_type {
                        ShapeType::Box => {
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                                ui.label("Rotation");
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                ui.add(egui::DragValue::new(&mut shape.0.rotation[2]).speed(0.01).clamp_range(-PI..=PI));
                                ui.add(egui::DragValue::new(&mut shape.0.rotation[1]).speed(0.01).clamp_range(-PI..=PI));
                                ui.add(egui::DragValue::new(&mut shape.0.rotation[0]).speed(0.01).clamp_range(-PI..=PI));
                            });
                            ui.end_row();

                            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                                ui.label("Size");
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                ui.add(egui::DragValue::new(&mut shape.0.size[2]).speed(0.1).clamp_range(0.0..=42069.0));
                                ui.add(egui::DragValue::new(&mut shape.0.size[1]).speed(0.1).clamp_range(0.0..=42069.0));
                                ui.add(egui::DragValue::new(&mut shape.0.size[0]).speed(0.1).clamp_range(0.0..=42069.0));
                            });
                            ui.end_row();
                        }
                        ShapeType::Sphere => {
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                                ui.label("Radius");
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                ui.add(egui::DragValue::new(&mut shape.0.size[0]).speed(0.1).clamp_range(0.0..=42069.0));
                            });
                            ui.end_row();
                        }
                        ShapeType::Cylinder => {
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                                ui.label("Height");
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                ui.add(egui::DragValue::new(&mut shape.0.size[1]).speed(0.1).clamp_range(0.0..=42069.0));
                            });
                            ui.end_row();

                            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                                ui.label("Radius");
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                ui.add(egui::DragValue::new(&mut shape.0.size[0]).speed(0.1).clamp_range(0.0..=42069.0));
                            });
                            ui.end_row();
                        }
                    }
                });
            });

        ui.separator();

        ui.end_row();
    }

    for id in delete_list {
        let pos = custom_shapes.iter().position(|s| s.0.id == id);
        if let Some(i) = pos { custom_shapes.remove(i); }
    }

    egui::Grid::new("util")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |_ui| {

            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.add(egui::Button::new("\u{2796}").min_size(egui::vec2(19.0, 19.0))).clicked() {
                    delete_mode ^= true;
                }
                if ui.add(egui::Button::new("\u{2795}").min_size(egui::vec2(19.0, 19.0))).clicked() {
                    // state.ui_state.events.push_back(UIEvent::CreateShape);
                    custom_shapes.push((Shape::new(), false));
                }
            });
            ui.end_row();
        });

    let mut ui_state = UISTATE.lock().unwrap();

    ui_state.delete_mode = delete_mode;
    ui_state.custom_shapes = custom_shapes;
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