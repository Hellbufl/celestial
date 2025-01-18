use std::collections::HashMap;
use std::{fs, time};
use std::vec::Vec;

use tracing::*;

use glam::{Vec3, Mat3};
use serde_binary::binary_stream;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use windows::Win32::Foundation::E_MBN_SMS_OPERATION_NOT_ALLOWED;

pub const DIRECT_COLLECTION_NAME : &str = "Direct Paths";
// pub const DEFAULT_COLLECTION_NAME : &str = "New Collection";

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct Path {
	id: Uuid,
	time: u64,
    nodes: Vec<[f32; 3]>,
}

impl Path {
    pub fn new() -> Path {
        Path {
            id: Uuid::new_v4(),
            time: 0,
            nodes: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn get_node(&self, index: usize) -> [f32; 3] {
        self.nodes[index]
    }

    pub fn nodes(&self) -> Vec<[f32; 3]> {
        self.nodes.clone()
    }

    pub fn set_time(&mut self, new_time: u64) {
        self.time = new_time;
    }

    pub fn time(&self) -> u64 {
        self.time
    }

    pub fn add_node(&mut self, pos: [f32; 3]) {
        self.nodes.push(pos);
    }

    // pub fn remove_node(&mut self, ) {} ?

    pub fn clear_nodes(&mut self) {
        self.nodes.clear();
    }

    pub fn from_file(file_path: String) -> Path {
        let file_content = fs::read(file_path).expect("[Celestial][PathLog] Error: failed to read path file!");
        serde_binary::from_vec(file_content, binary_stream::Endian::Little).expect("[Celestial][PathLog] Error: failed to decode path file!")
    }

	pub fn to_file(&mut self, file_path: String) {
        fs::write(
            file_path, serde_binary::to_vec(self, binary_stream::Endian::Little).expect("[Celestial][PathLog] Error: failed to serialize path file!")
        ).expect("[Celestial][PathLog] Error: failed to write path file!");
    }
}

impl PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

#[derive(Clone, Copy)]
#[derive(Serialize, Deserialize)]
pub struct BoxCollider {
	position: Vec3,
	rotation: [f32; 3],
    basis: Mat3,
    size: [f32; 3],
}

impl BoxCollider {
    pub fn new(pos: [f32; 3], rotation: [f32; 3], size: [f32; 3]) -> BoxCollider {
        BoxCollider {
            position: pos.into(),
            rotation,
            basis: Mat3::from_euler(glam::EulerRot::XYZ, rotation[0], rotation[1], rotation[2]).transpose(), // transposing cause xA = A^Tx and i can't do Vec3 * Mat3
            size,
        }
    }

    pub fn position(&self) -> [f32; 3] {
        self.position.into()
    }

    pub fn basis(&self) -> [[f32; 3]; 3] {
        self.basis.to_cols_array_2d()
    }

    pub fn size(&self) -> [f32; 3] {
        self.size
    }

    pub fn rotation(&self) -> [f32; 3] {
        // let (rx, ry, rz) = self.basis.to_euler(glam::EulerRot::XYZ); // dunno why the angles are wrong. not just wrong range but this does weird flippery stuff
        // [rx, ry, rz]
        self.rotation
    }

    pub fn check_point_collision(&mut self, point: Vec3) -> bool {
        let relative_pos = self.basis * (point - self.position);
		relative_pos.x.abs() <= self.size[0] && relative_pos.y.abs() <= self.size[1] && relative_pos.z.abs() <= self.size[2]
    }
}

pub enum HighPassFilter {
    GOLD,
    PATH {
        id: Uuid,
    },
}

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct PathCollection {
    id: Uuid,
    pub name: String,
    paths: Vec<Path>,
}

impl PathCollection {
    pub fn new(new_name: String) -> PathCollection {
        PathCollection {
            id: Uuid::new_v4(),
            name: new_name,
            paths: Vec::new(),
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    // pub fn get_path(&self, index: usize) -> Path {
    //     self.paths[index].clone()
    // }

    pub fn get_path(&self, id: Uuid) -> Option<&Path> {
        match self.paths.iter().position(|p| p.id() == id) {
            Some(i) => Some(&self.paths[i]),
            None => None,
        }
    }

    pub fn paths(&self) -> &Vec<Path> {
        &self.paths
    }

    pub fn add(&mut self, new_path: Path, high_pass: Option<&HighPassFilter>) {
        match high_pass {
            Some(HighPassFilter::GOLD) => {
                if self.paths.len() == 0 || self.paths[0].time() > new_path.time() {
                    self.paths.insert(0, new_path);
                }
                return;
            }
            Some(HighPassFilter::PATH { id }) => {
                if self.paths.is_empty() {
                    self.paths.push(new_path);
                    return;
                }

                for i in 0..self.paths.len() {
                    if self.paths[i].time() > new_path.time() {
                        self.paths.insert(i, new_path);
                        return;
                    }
                    if self.paths[i].id() == *id { return; }
                }
            }
            None => {
                for i in 0..self.paths.len() {
                    if self.paths[i].time() < new_path.time() { continue; }
                    self.paths.insert(i, new_path);
                    return;
                }
                self.paths.push(new_path);
            }
        }
    }

    pub fn remove(&mut self, id: Uuid) {
        if let Some(index) = self.paths.iter().position(|p| p.id == id) {
            self.paths.remove(index);
        }
    }

    // pub fn sort(&mut self) {} ?
}

#[derive(Serialize, Deserialize)]
pub struct CompFile {
    trigger_data: [[[f32; 3]; 3]; 2],
    collections: Vec<PathCollection>,
}

impl CompFile {

    // for some reason glam vectors don't deserialize correctly with serde_binary
    // so i have to convert them from and to arrays myself
    pub fn new(trigger: [BoxCollider; 2], collections: Vec<PathCollection>) -> CompFile {

        let trigger_data = [[
                trigger[0].position.to_array(),
                trigger[0].rotation(),
                trigger[0].size,
            ], [
                trigger[1].position.to_array(),
                trigger[1].rotation(),
                trigger[1].size,
            ]
        ];

        CompFile{
            trigger_data,
            collections,
        }
    }

    pub fn get_triggers(&self) -> [Option<BoxCollider>; 2] {
        [
            Some(BoxCollider::new(
                self.trigger_data[0][0],
                self.trigger_data[0][1],
                self.trigger_data[0][2],
            )),
            Some(BoxCollider::new(
                self.trigger_data[1][0],
                self.trigger_data[1][1],
                self.trigger_data[1][2],
            ))
        ]
    }

    pub fn get_collections(&self) -> Vec<PathCollection> {
        self.collections.clone()
    }

    pub fn from_file(file_path: String) -> CompFile {
        let file_content = fs::read(file_path).expect("[Celestial][PathLog] Error: failed to read comparison file!");
        serde_binary::from_vec(file_content, binary_stream::Endian::Little).expect("[Celestial][PathLog] Error: failed to decode comparison file!")
    }

	pub fn to_file(&self, file_path: String) {
        fs::write(
            file_path, serde_binary::to_vec(self, binary_stream::Endian::Little).expect("[Celestial][PathLog] Error: failed to serialize comparison file!")
        ).expect("[Celestial][PathLog] Error: failed to write comparison file!");
    }
}

pub struct PathLog {
	primed: bool,
	recording: bool,
	direct: bool,
	autosave: bool,

    current_file: Option<String>,
	recording_start: Option<time::Instant>,
    pub latest_path: Uuid,
    latest_time: u64,
	pub recording_path: Path,
	pub direct_paths: PathCollection,
    pub path_collections: Vec<PathCollection>,
    // pub active_collections: Vec<Uuid>,
    pub active_collection: Option<Uuid>,
    pub triggers: [Option<BoxCollider>; 2],
    pub filters: HashMap<Uuid, HighPassFilter>,
}

impl PathLog {
    pub fn new() -> PathLog {
        let pathlog = PathLog {
            primed: false,
            recording: false,
            direct: false,
            autosave: false,

            current_file: None,

            recording_start: None,
            latest_path: Uuid::new_v4(),
            latest_time: 0,
            recording_path: Path::new(),
            direct_paths: PathCollection::new(DIRECT_COLLECTION_NAME.to_string()),
            path_collections: Vec::new(),
            // active_collections: Vec::new(),
            active_collection: None,
            triggers: [None, None],
            filters: HashMap::new(),
        };

        if !std::fs::exists("Paths").expect("wtf bro where is your dll") && std::fs::create_dir("Paths").is_err() {
            error!("Failed to create Paths directory!");
            std::process::exit(1);
        }
    
        info!("Initialized");

        pathlog
    }

	pub fn update(&mut self, player_pos: &[f32; 3], player_rot: &[f32; 3]) {
        let player_up = Mat3::from_euler(glam::EulerRot::XYZ, player_rot[0], player_rot[1], player_rot[2]) * Vec3::Y;
        let player_center = [
            player_pos[0] + player_up.x,
            player_pos[1] + player_up.y,
            player_pos[2] + player_up.z
        ];

        if let [Some(start_trigger), Some(finish_trigger)] = self.triggers.as_mut() {
            let player_in_trigger = [
                start_trigger.check_point_collision(player_center.into()),
                finish_trigger.check_point_collision(player_center.into())
            ];
            
            if player_in_trigger[0] && !self.primed {
                self.reset();
                self.primed = true;
            }
            else if !player_in_trigger[0] && self.primed {
                self.primed = false;
                self.start();
            }

            if player_in_trigger[1] && self.recording {
                self.stop();
            }
        }

        if !self.recording { return; }
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
        self.recording_path.clear_nodes();
        self.recording_path.set_time(0);
        self.recording_start = None;
        info!("Recording reset");
    }

	pub fn stop(&mut self) {
        if !self.recording { return; }

        self.recording = false;

        let time_recorded = self.recording_start.unwrap().elapsed().as_millis() as u64;
        self.recording_path.set_time(time_recorded);
        self.latest_time = time_recorded;

        if self.direct {
            self.direct_paths.add(self.recording_path.clone(), None);
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
            rec_start.elapsed().as_millis() as u64
        } else { self.latest_time }
    }

    pub fn set_direct_mode(&mut self, mode: bool) {
        self.direct = mode;
    }

    pub fn set_autosave(&mut self, mode: bool) {
        self.autosave = mode;
    }

	pub fn insert(&mut self, new_path: &Path, collection_id: Uuid) {
        if let Some(index) = self.path_collections.iter().position(|coll| coll.id() == collection_id) {
            self.path_collections[index].add(new_path.clone(), self.filters.get(&collection_id));
        }
        else if self.direct_paths.id() == collection_id {
            self.direct_paths.add(new_path.clone(), None);
        }
        else { error!("Collection-ID '{collection_id}' does not exist!") }
    }

    pub fn remove(&mut self, path_id: Uuid, collection_id: Uuid) {
        if let Some(index) = self.path_collections.iter().position(|coll| coll.id() == collection_id) {
            self.path_collections[index].remove(path_id);
        }
        else if self.direct_paths.id() == collection_id {
            self.direct_paths.remove(path_id);
        }
        else { error!("Collection-ID '{collection_id}' does not exist!") }
    }

    pub fn load_comparison(&mut self, file_path: String) {
        let data = CompFile::from_file(file_path);
        self.triggers = data.get_triggers();
        self.path_collections = data.get_collections();
    }

    pub fn save_comparison(&mut self, file_path: String) {
        if self.triggers[0].is_none() { return; };
        if self.triggers[1].is_none() { return; };

        let data = CompFile::new(
            [
                self.triggers[0].as_ref().unwrap().clone(),
                self.triggers[1].as_ref().unwrap().clone()
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
        self.triggers[index] = Some(BoxCollider::new(player_center, rotation, size));
    }

	pub fn clear_triggers(&mut self) {
        for collection in &self.path_collections {
            if !collection.paths.is_empty() { return; }
        }
        self.triggers = [None, None];
    }
}