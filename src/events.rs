use std::collections::VecDeque;
use uuid::Uuid;
use std::sync::mpsc;
use std::thread;
use native_dialog::FileDialog;

use tracing::*;
use crate::{gamedata, RenderUpdates, CONFIG_STATE, EVENTS, PATHLOG, RENDER_UPDATES, UISTATE, RX};
use crate::pathdata::{HighPassFilter, FILE_EXTENTION};
use crate::config::CONFIG_FILE_NAME;
use crate::ui::{Teleport, TeleportIndex};

#[derive(Clone)]
pub enum CelEvent {
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
    DeleteTrigger {
        id: Uuid,
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
        index: TeleportIndex,
    },
    SpawnTeleport {
        index: TeleportIndex,
    },
    RenderUpdate {
        update: RenderUpdates,
    }
}

pub fn process_events() {
    let mut loop_events : VecDeque<CelEvent> = VecDeque::new();

    let mut events = EVENTS.lock().unwrap();

    let mut event_list = events.clone();
    events.clear();

    drop(events);

    while let Some(event) = event_list.pop_front() {
        match event {
            CelEvent::DeletePath { path_id } => {
                PATHLOG.lock().unwrap().delete_path(path_id);
                loop_events.push_back(CelEvent::RenderUpdate { update: RenderUpdates::paths() });
            }
            CelEvent::ChangeDirectMode { new } => {
                PATHLOG.lock().unwrap().set_direct_mode(new);
            }
            CelEvent::ChangeAutosave { new } => {
                PATHLOG.lock().unwrap().set_autosave(new);
            }
            CelEvent::ChangeAutoReset { new } => {
                PATHLOG.lock().unwrap().set_autoreset(new);
            }
            CelEvent::SpawnTrigger { index, position, rotation } => {
                let trigger_size = CONFIG_STATE.lock().unwrap().trigger_sizes[index];
                if PATHLOG.lock().unwrap().is_empty() {
                    PATHLOG.lock().unwrap().create_trigger(index, position, rotation, trigger_size);
                    EVENTS.lock().unwrap().push_back(CelEvent::SpawnTeleport { index: TeleportIndex::Main { i: index } });
                }
                else {
                    // TODO: popup warning
                }
            }
            CelEvent::DeleteTrigger { id } => {
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
            CelEvent::StartRecording => {
                PATHLOG.lock().unwrap().start();
            }
            CelEvent::StopRecording => {
                let mut pathlog = PATHLOG.lock().unwrap();

                let recording_path_id = pathlog.recording_path.id();
                pathlog.mute_paths.insert(recording_path_id, false);
                pathlog.solo_paths.insert(recording_path_id, false);
                pathlog.stop();

                drop(pathlog);

                loop_events.push_back(CelEvent::RenderUpdate { update: RenderUpdates::paths() });
            }
            CelEvent::ResetRecording => {
                PATHLOG.lock().unwrap().reset();
            }
            CelEvent::ClearTriggers => {
                PATHLOG.lock().unwrap().clear_triggers();
                UISTATE.lock().unwrap().main_teleports = [None; 2];
            }
            CelEvent::CreateCollection => {
                PATHLOG.lock().unwrap().create_collection();
            }
            CelEvent::RenameCollection { id, new_name } => {
                PATHLOG.lock().unwrap().rename_collection(id, new_name);
            }
            CelEvent::DeleteCollection { id } => {
                PATHLOG.lock().unwrap().delete_collection(id);
                loop_events.push_back(CelEvent::RenderUpdate { update: RenderUpdates::paths() });
            }
            CelEvent::ToggleMute { id } => {
                let mut pathlog = PATHLOG.lock().unwrap();

                if let Some(b) = pathlog.mute_paths.get_mut(&id) { *b ^= true; }
                if let Some(b) = pathlog.mute_collections.get_mut(&id) { *b ^= true; }
                pathlog.update_visible();

                drop(pathlog);

                loop_events.push_back(CelEvent::RenderUpdate { update: RenderUpdates::paths() });
            },
            CelEvent::ToggleSolo { id } => {
                let mut pathlog = PATHLOG.lock().unwrap();

                if let Some(b) = pathlog.solo_paths.get_mut(&id) { *b ^= true; }
                if let Some(b) = pathlog.solo_collections.get_mut(&id) { *b ^= true; }
                pathlog.update_visible();

                drop(pathlog);

                loop_events.push_back(CelEvent::RenderUpdate { update: RenderUpdates::paths() });
            },
            CelEvent::ToggleActive { id } => {
                let mut pathlog = PATHLOG.lock().unwrap();

                if pathlog.active_collection == Some(id) {
                    pathlog.active_collection = None;
                }
                else {
                    pathlog.active_collection = Some(id);
                }

                drop(pathlog);
            }
            CelEvent::ToggleGoldFilter { collection_id } => {
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
            CelEvent::SetPathFilter { collection_id, path_id } => {
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
            CelEvent::SaveComparison => {
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
                    loop_events.push_back(CelEvent::SaveComparison);
                }
                else if let Some(RX::Save { rx }) = &ui_state.file_path_rx {
                    if let Ok(dialog_result) = rx.try_recv() {
                        drop(ui_state);

                        if let Ok(Some(path)) = dialog_result {
                            PATHLOG.lock().unwrap().save_comparison(path.to_str().unwrap().to_string());
                        }
                        UISTATE.lock().unwrap().file_path_rx = None;
                    }
                    else { loop_events.push_back(CelEvent::SaveComparison); }
                }
            }
            CelEvent::LoadComparison => {
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
                    loop_events.push_back(CelEvent::LoadComparison);
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
                        loop_events.push_back(CelEvent::RenderUpdate { update: RenderUpdates::paths() });
                    }
                    else { loop_events.push_back(CelEvent::LoadComparison); }
                }
            }
            CelEvent::SaveConfig => {
                if let Err(e) = CONFIG_STATE.lock().unwrap().write(CONFIG_FILE_NAME.to_string()) {
                    error!("{e}");
                }
            },
            CelEvent::LoadConfig => {
                if let Err(e) = CONFIG_STATE.lock().unwrap().read(CONFIG_FILE_NAME.to_string()) {
                    error!("{e}");
                }
            },
            CelEvent::SelectPath { path_id, collection_id, modifier } => {
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
                loop_events.push_back(CelEvent::RenderUpdate { update: RenderUpdates::paths() });
            }
            CelEvent::Teleport { index } => {
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
                    loop_events.push_back(CelEvent::ResetRecording);
                }
            }
            CelEvent::SpawnTeleport { index } => {
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
            CelEvent::RenderUpdate { update } => {
                RENDER_UPDATES.lock().unwrap().or(update);

                if update.paths {
                    PATHLOG.lock().unwrap().update_visible();
                }
            }
        }
    }

    EVENTS.lock().unwrap().append(&mut loop_events);
}