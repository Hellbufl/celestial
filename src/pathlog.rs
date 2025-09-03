use std::collections::HashMap;
use std::time;
use std::vec::Vec;

use tracing::{error, info};

use glam::{Vec3, Mat3};
use uuid::Uuid;

use crate::error::Error;
use crate::{pathdata::*, RenderUpdates};

pub const DEFAULT_COLLECTION_NAME : &str = "New Collection";
// pub const DIRECT_COLLECTION_NAME : &str = "Direct Paths";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ComparisonMode {
    All,
    Gold,
    Median,
}

#[derive(Clone, Copy)]
pub struct Comparison {
    pub mode: ComparisonMode,
    pub len: usize,
}

pub struct PathLog {
    paused: bool,
	primed: bool,
	recording: bool,
	direct: bool,
	autosave: bool,
    autoreset: bool,

    current_file: Option<String>,
	recording_start: Option<time::Instant>,
    pub latest_path: Uuid,
    latest_time: u64,
	pub recording_path: Path,
    pub active_collection: Option<Uuid>,
    pub filters: HashMap<Uuid, HighPassFilter>,

    pub main_triggers: [Option<BoxCollider>; 2],
    pub checkpoint_triggers: Vec<BoxCollider>,

    paths: HashMap<Uuid, Path>,

    path_collections: Vec<PathCollection>,
	// pub direct_paths: PathCollection,

    pub mute_paths: HashMap<Uuid, bool>,
    pub solo_paths: HashMap<Uuid, bool>,
    pub mute_collections: HashMap<Uuid, bool>,
    pub solo_collections: HashMap<Uuid, bool>,
    pub selected_paths: HashMap<Uuid, Vec<Uuid>>,

    comparison: Comparison,
    // compared_paths: PathCollection,
    compared_paths: Vec<(Uuid, usize)>,
    ignored_paths: Vec<Vec<Uuid>>,
}

impl PathLog {
    pub fn init() -> PathLog {
        let pathlog = PathLog {
            paused: false,
            primed: false,
            recording: false,
            direct: false,
            autosave: false,
            autoreset: true,

            current_file: None,

            recording_start: None,
            latest_path: Uuid::new_v4(),
            latest_time: 0,
            recording_path: Path::new(),
            active_collection: None,
            filters: HashMap::new(),

            main_triggers: [None, None],
            checkpoint_triggers: Vec::new(),

            paths: HashMap::new(),
            path_collections: Vec::new(),

            mute_paths: HashMap::new(),
            solo_paths: HashMap::new(),
            mute_collections: HashMap::new(),
            solo_collections: HashMap::new(),
            selected_paths: HashMap::new(),

            comparison: Comparison { mode: ComparisonMode::All, len: 0 },
            // compared_paths: PathCollection::new("compared".to_string()),
            compared_paths: Vec::new(),
            ignored_paths: Vec::new(),
        };

        if !std::fs::exists("Paths").expect("") && std::fs::create_dir("Paths").is_err() {
            error!("Failed to create Paths directory!");
            std::process::exit(1);
        }

        info!("Initialized");

        pathlog
    }

	pub fn update(&mut self, player_pos: &[f32; 3], player_rot: &[f32; 3]) -> RenderUpdates {
        let mut updates = RenderUpdates::new();

        let player_up = Mat3::from_euler(glam::EulerRot::XYZ, player_rot[0], player_rot[1], player_rot[2]) * Vec3::Y;
        let player_center = [
            player_pos[0] + player_up.x,
            player_pos[1] + player_up.y,
            player_pos[2] + player_up.z
        ];

        if let [Some(start_trigger), Some(finish_trigger)] = self.main_triggers.as_mut() {
            let player_in_trigger = [
                start_trigger.check_point_collision(player_center.into()),
                finish_trigger.check_point_collision(player_center.into())
            ];

            if player_in_trigger[0] && !self.primed && self.autoreset {
                self.reset();
                self.primed = true;
            }
            else if !player_in_trigger[0] && self.primed {
                self.primed = false;
                self.start();
            }

            if player_in_trigger[1] && self.recording {
                self.stop();
                updates.paths = true;
            }
        }

        if !self.recording || self.paused { return updates; }

        // TODO: checkpoint logic

	    self.recording_path.add_node(player_center);

        updates
    }

    pub fn update_visible(&mut self) {
        self.compared_paths.clear();
        self.ignored_paths.clear();

        let mut all_compared : Vec<Uuid> = Vec::new();

        for path in self.paths.values() {
            if let Some(pos) = all_compared.iter().position(|id| path.time() < self.path(&id).unwrap().time()) {
                all_compared.insert(pos, path.id());
            }
            else {
                all_compared.push(path.id());
            }
        }

        self.comparison.len = all_compared.len();

        for c in 0..self.path_collections.len() {
            let collection = &self.path_collections[c];

            self.ignored_paths.push(Vec::new());

            if collection.paths().is_empty() { continue; }

            let mut collection_visible = self.solo_collections.values().all(|s| !s);
            if *self.solo_collections.get(&collection.id()).unwrap() { collection_visible = true; }
            if *self.mute_collections.get(&collection.id()).unwrap() { collection_visible = false; }

            // if visible { continue; }

            if matches!(self.comparison.mode, ComparisonMode::Gold) {
                let position =  all_compared.iter().position(|id| *id == collection.paths()[0]).unwrap();
                self.compared_paths.push((collection.paths()[0], position));
                continue;
            }

            if matches!(self.comparison.mode, ComparisonMode::Median) {
                let position =  all_compared.iter().position(|id| *id == collection.paths()[collection.paths().len() / 2]).unwrap();
                self.compared_paths.push((collection.paths()[collection.paths().len() / 2], position));
            }

            for i in 0..collection.paths().len() {
                let path_id = collection.paths()[i];
                let mut path_visible = self.solo_paths.values().all(|s| !s);
                if *self.solo_paths.get(&path_id).unwrap() { path_visible = true; }
                if *self.mute_paths.get(&path_id).unwrap() { path_visible = false; }

                if !collection_visible || !path_visible { continue; }

                if matches!(self.comparison.mode, ComparisonMode::Median) {
                    if self.compared_paths.iter().find(|x| x.0 == path_id).is_some() { continue; }
                    self.ignored_paths[c].push(path_id);
                    continue;
                }

                let position = all_compared.iter().position(|id| *id == path_id).unwrap();
                self.compared_paths.push((path_id, position));
            }
        }
    }

    fn add_path_to_collection(&mut self, path_id: Uuid, collection_id: Uuid) {
        let collection = self.path_collections.iter_mut().find(|c| c.id() == collection_id).unwrap();
        let new_path = self.paths.get(&path_id).unwrap();
        let mut position = collection.paths().len();

        if !collection.paths().is_empty() {
            match self.filters.get(&collection_id) {
                Some(HighPassFilter::Gold) => {
                    // if self.paths.get(&collection.paths()[0]).unwrap().time() > new_path.time() {
                    //     collection.insert(0, path_id);
                    // }
                    if self.paths.get(&collection.paths()[0]).unwrap().time() < new_path.time() {
                        return;
                    }
                    position = 0;
                }
                Some(HighPassFilter::Path { id }) => {
                    for i in 0..collection.paths().len() {
                        if self.paths.get(&collection.paths()[i]).unwrap().time() > new_path.time() {
                            // collection.insert(i, path_id);
                            position = i;
                            break;
                        }
                        if self.paths.get(&collection.paths()[i]).unwrap().id() == *id { return; }
                    }
                }
                None => {
                    for i in 0..collection.paths().len() {
                        if self.paths.get(&collection.paths()[i]).unwrap().time() < new_path.time() { continue; }
                        // collection.insert(i, path_id);
                        position = i;
                        break;
                    }
                    // collection.push(path_id);
                }
            }
        }

        self.mute_paths.entry(path_id).or_insert(false);
        self.solo_paths.entry(path_id).or_insert(false);

        collection.insert(position, path_id);
    }

    pub fn compared_paths(&self) -> &Vec<(Uuid, usize)> {
        &self.compared_paths
    }

    pub fn ignored_paths(&self) -> &Vec<Vec<Uuid>> {
        &self.ignored_paths
    }

    pub fn comparison(&self) -> Comparison {
        self.comparison
    }

    pub fn path(&self, path_id: &Uuid) -> Option<&Path> {
        self.paths.get(path_id)
    }

	pub fn start(&mut self) {
        if self.recording { return; }
        self.recording = true;
        self.recording_start = Some(time::Instant::now());
        info!("Recording started");
    }

	pub fn reset(&mut self) {
        if !self.recording { return; }
        self.recording = false;
        self.recording_path.clear_all();
        // self.recording_path.set_time(0);
        self.recording_start = None;
        info!("Recording reset");
    }

    // pub fn split(&mut self) {

    // }

    pub fn pause(&mut self) {
        if !self.recording || self.paused { return; }

        let segment_time = self.recording_start.unwrap().elapsed().as_millis() as u64;
        self.recording_path.end_segment(segment_time);

        self.paused = true;
        info!("Recording paused");
    }

    pub fn unpause(&mut self) {
        if !self.recording || !self.paused { return; }

        self.recording_start = Some(time::Instant::now());

        self.paused = false;
        info!("Recording unpaused");
    }

    pub fn toggle_pause(&mut self) {
        if self.paused {
            self.unpause();
        }
        else {
            self.pause();
        }
    }

	pub fn stop(&mut self) {
        if !self.recording { return; }

        self.recording = false;

        let time_recorded = self.recording_start.unwrap().elapsed().as_millis() as u64;
        self.recording_path.end_path(time_recorded);
        self.latest_time = self.recording_path.time();

        if self.direct {
            // self.direct_paths.add(self.recording_path.clone(), None);
        }
        else {
            let mut empty = true;

            for i in 0..self.path_collections.len() {
                empty &= self.path_collections[i].paths().is_empty();

                let id = self.path_collections[i].id();
                if self.active_collection == Some(id) {
                    self.paths.insert(self.recording_path.id(), self.recording_path.clone());
                    self.add_path_to_collection(self.recording_path.id(), id);
                }
            }

            if self.autosave && !empty {
                if let Some(file_path) = &self.current_file {
                    self.save_comparison(file_path.clone());
                }
            }
        }

        self.latest_path = self.recording_path.id();

        self.recording_path = Path::new();
        self.recording_start = None;

        self.update_visible();

        info!("Recording stopped");
    }

    pub fn time(&self) -> u64 {
        if let Some(rec_start) = self.recording_start {
            let mut current_time : u64 = 0;
            if !self.paused {
                current_time = rec_start.elapsed().as_millis() as u64;
            }
            current_time + self.recording_path.time()
        } else { self.latest_time }
    }

    pub fn set_direct_mode(&mut self, mode: bool) {
        self.direct = mode;
    }

    pub fn set_autosave(&mut self, mode: bool) {
        self.autosave = mode;
    }

    pub fn set_autoreset(&mut self, mode: bool) {
        self.autoreset = mode;
    }

    pub fn set_comparison_mode(&mut self, mode: ComparisonMode) {
        self.comparison.mode = mode;
    }

	// pub fn insert(&mut self, new_path: &Path, collection_id: Uuid) {
    //     if let Some(collection) = self.path_collections.iter_mut().find(|coll| coll.id() == collection_id) {
    //         collection.add(new_path.clone(), self.filters.get(&collection_id));
    //     }
    //     else { error!("Collection-ID '{collection_id}' does not exist!") }
    // }

    // pub fn remove(&mut self, path_id: Uuid, collection_id: Uuid) {
    //     if let Some(index) = self.path_collections.iter().position(|coll| coll.id() == collection_id) {
    //         self.path_collections[index].remove(path_id);
    //         self.update_visible();
    //     }
    //     else { error!("Collection-ID '{collection_id}' does not exist!") }
    // }

    pub fn collections(&self) -> &std::vec::Vec<PathCollection> {
        &self.path_collections
    }

    pub fn get_collection(&self, collection_id: Uuid) -> Option<&PathCollection> {
        self.path_collections.iter().find(|c| c.id() == collection_id)
    }

    pub fn delete_path(&mut self, path_id: Uuid) {
        for collection in &mut self.path_collections {
            if let Some(HighPassFilter::Path{ id: filter_id }) = self.filters.get(&collection.id()) {
                if *filter_id == path_id {
                    self.filters.remove(&collection.id());
                }
            }

            collection.remove(path_id);
        }

        self.mute_paths.remove(&path_id);
        self.solo_paths.remove(&path_id);
        self.paths.remove(&path_id);
        self.update_visible();
    }

    pub fn create_collection(&mut self) {
        let new_collection = PathCollection::new(DEFAULT_COLLECTION_NAME.to_string());

        if self.path_collections.is_empty() {
            self.active_collection = Some(new_collection.id());
        }

        self.mute_collections.insert(new_collection.id(), false);
        self.solo_collections.insert(new_collection.id(), false);
        self.selected_paths.insert(new_collection.id(), Vec::new());
        self.path_collections.push(new_collection);
    }

    pub fn rename_collection(&mut self, collection_id: Uuid, mut new_name: String) {
        if new_name == "" { new_name = DEFAULT_COLLECTION_NAME.to_string() }
        if let Some(collection) = self.path_collections.iter_mut().find(|c| c.id() == collection_id) {
            collection.name = new_name;
        }
    }

    pub fn delete_collection(&mut self, collection_id: Uuid) {
        if let Some(index) = self.path_collections.iter().position(|coll| coll.id() == collection_id) {
            for path_id in self.path_collections[index].paths() {
                self.mute_paths.remove(&path_id);
                self.solo_paths.remove(&path_id);
                self.paths.remove(path_id);
            }

            if self.active_collection == Some(collection_id) {
                self.active_collection = None;
            }

            self.mute_collections.remove(&collection_id);
            self.solo_collections.remove(&collection_id);
            self.path_collections.remove(index);
            self.update_visible();
        }
        else { error!("Collection-ID '{collection_id}' does not exist!") }
    }

    pub fn is_empty(&self) -> bool {
        let mut empty = true;
        for collection in &self.path_collections {
            empty &= collection.paths().is_empty();
        }
        empty
    }

    pub fn load_comparison(&mut self, file_path: String) -> Result<(), Error> {
        let data = CompFile::from_file(file_path)?;
        self.main_triggers = data.get_triggers();
        self.paths = data.get_paths();
        self.path_collections = data.get_collections();
        self.current_file = None;
        self.active_collection = None;

        self.mute_collections.clear();
        self.solo_collections.clear();
        self.selected_paths.clear();
        self.mute_paths.clear();
        self.solo_paths.clear();

        for collection in &self.path_collections {
            self.mute_collections.insert(collection.id(), false);
            self.solo_collections.insert(collection.id(), false);
            self.selected_paths.insert(collection.id(), Vec::new());

            for path_id in collection.paths() {
                self.mute_paths.insert(*path_id, false);
                self.solo_paths.insert(*path_id, false);
            }
        }

        self.update_visible();
        Ok(())
    }

    pub fn save_comparison(&mut self, file_path: String) {
        if self.main_triggers[0].is_none() { return; };
        if self.main_triggers[1].is_none() { return; };

        let data = CompFile::new(
            [
                self.main_triggers[0].as_ref().unwrap().clone(),
                self.main_triggers[1].as_ref().unwrap().clone()
            ],

            self.paths.clone(),
            self.path_collections.clone(),
        );

        if let Err(e) = data.to_file(file_path.clone()) {
            error!("{e}");
        }

        self.current_file = Some(file_path);
    }

    pub fn create_trigger(&mut self, index: usize, position: [f32; 3], rotation: [f32; 3], size: [f32; 3]) {
        let player_basis = Mat3::from_euler(glam::EulerRot::XYZ, rotation[0], rotation[1], rotation[2]);
        let player_up = player_basis.transpose() * Vec3::Y;
        let player_center = [
            position[0] + player_up.x,
            position[1] + player_up.y,
            position[2] + player_up.z
        ];
        self.current_file = None;

        if index < 2 {
            self.main_triggers[index] = Some(BoxCollider::new(player_center, rotation, size));
        }
        else {
            self.checkpoint_triggers.push(BoxCollider::new(player_center, rotation, size));
        }
    }

	pub fn clear_triggers(&mut self) {
        for collection in &self.path_collections {
            if !collection.paths().is_empty() { return; }
        }
        self.main_triggers = [None, None];
        self.current_file = None;
    }
}