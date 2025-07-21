use std::collections::HashMap;
use std::time;
use std::vec::Vec;

use tracing::{error, info};

use glam::{Vec3, Mat3};
use uuid::Uuid;

use crate::error::Error;
use crate::{pathdata::*, RenderUpdates};

pub const DIRECT_COLLECTION_NAME : &str = "Direct Paths";

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
    pub path_collections: Vec<PathCollection>,
	// pub direct_paths: PathCollection,
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
            path_collections: Vec::new(),
            // direct_paths: PathCollection::new(DIRECT_COLLECTION_NAME.to_string()),
        };

        if !std::fs::exists("Paths").expect("") && std::fs::create_dir("Paths").is_err() {
            error!("Failed to create Paths directory!");
            std::process::exit(1);
        }

        info!("Initialized");

        pathlog
    }

	pub fn update(&mut self, player_pos: &[f32; 3], player_rot: &[f32; 3], updates: &mut RenderUpdates) {
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

        if !self.recording || self.paused { return; }

        // TODO: checkpoint logic

	    self.recording_path.add_node(player_center);
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
        // let p : Option<i32> = None;
        // let _a = p.unwrap();
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
                    self.path_collections[i].add(self.recording_path.clone(), self.filters.get(&id));
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

	pub fn insert(&mut self, new_path: &Path, collection_id: Uuid) {
        if let Some(index) = self.path_collections.iter().position(|coll| coll.id() == collection_id) {
            self.path_collections[index].add(new_path.clone(), self.filters.get(&collection_id));
        }
        // else if self.direct_paths.id() == collection_id {
        //     self.direct_paths.add(new_path.clone(), None);
        // }
        else { error!("Collection-ID '{collection_id}' does not exist!") }
    }

    pub fn remove(&mut self, path_id: Uuid, collection_id: Uuid) {
        if let Some(index) = self.path_collections.iter().position(|coll| coll.id() == collection_id) {
            self.path_collections[index].remove(path_id);
        }
        // else if self.direct_paths.id() == collection_id {
        //     self.direct_paths.remove(path_id);
        // }
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
        self.path_collections = data.get_collections();
        self.current_file = None;
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

            self.path_collections.clone()
        );

        data.to_file(file_path.clone());
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