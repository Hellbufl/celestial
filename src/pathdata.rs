use std::collections::HashMap;
use std::fs;
use std::vec::Vec;
// use tracing::{error, info};
use glam::{Vec3, Mat3};
use serde_binary::binary_stream;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

use crate::error::Error;

const CURRENT_FILE_VERSION : &str = "0.5.1";

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct Path {
	id: Uuid,
	times: Vec<u64>,
    segments: Vec<Vec<[f32; 3]>>,
}

impl Path {
    pub fn new() -> Path {
        Path {
            id: Uuid::new_v4(),
            times: Vec::new(),
            segments: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        let mut sum : usize = 0;
        for segment in &self.segments {
            sum += segment.len();
        }
        sum
    }

    pub fn segment_len(&self, index: usize) -> Option<usize> {
        if index < self.segments.len() {
            Some(self.segments[index].len())
        }
        else { None }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn get_node(&self, segment: usize, index: usize) -> Option<[f32; 3]> {
        if segment < self.segments.len() && index < self.segments[segment].len() {
            Some(self.segments[segment][index])
        }
        else { None }
    }

    pub fn segments(&self) -> Vec<Vec<[f32; 3]>> {
        self.segments.clone()
    }

    pub fn segment_nodes(&self, index: usize) -> Option<Vec<[f32; 3]>> {
        if index < self.segments.len() {
            Some(self.segments[index].clone())
        }
        else { None }
    }

    pub fn end_segment(&mut self, time: u64) {
        self.segments.push(Vec::new());
        self.times.push(time);
    }

    pub fn end_path(&mut self, time: u64) {
        self.times.push(time);
    }

    pub fn time(&self) -> u64 {
        self.times.iter().sum()
    }

    pub fn segment_time(&self, index: usize) -> Option<u64> {
        if index < self.times.len() {
            Some(self.times[index])
        }
        else { None }
    }

    pub fn add_node(&mut self, pos: [f32; 3]) {
        if self.segments.is_empty() {
            self.segments.push(Vec::new());
        }
        let last = self.segments.len() - 1;

        if self.segment_time(last).is_none() {
            self.segments[last].push(pos);
        }
    }

    pub fn clear_all(&mut self) {
        self.segments.clear();
        self.times.clear();
    }
}

impl PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl std::fmt::Debug for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("Path")
         .field("id", &self.id)
         .finish()
    }
}

#[derive(Clone, Copy)]
#[derive(Serialize, Deserialize)]
pub struct BoxCollider {
    id: Uuid,
	// position: Vec3,
	pub position: [f32; 3],
	rotation: [f32; 3],
    basis: Mat3,
    pub size: [f32; 3],
}

impl BoxCollider {
    pub fn new(pos: [f32; 3], rotation: [f32; 3], size: [f32; 3]) -> BoxCollider {
        BoxCollider {
            id: Uuid::new_v4(),
            position: pos.into(),
            rotation,
            basis: Mat3::from_euler(glam::EulerRot::XYZ, rotation[0], rotation[1], rotation[2]).transpose(),
            size,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn basis(&self) -> [[f32; 3]; 3] {
        self.basis.to_cols_array_2d()
    }

    pub fn rotation(&self) -> [f32; 3] {
        // let (rx, ry, rz) = self.basis.to_euler(glam::EulerRot::XYZ); // dunno why the angles are wrong. not just wrong range but this does weird flippery stuff
        // [rx, ry, rz]
        self.rotation
    }

    pub fn set_rotation(&mut self, new_rot: [f32;3]) {
        self.rotation = new_rot;
        self.basis = Mat3::from_euler(glam::EulerRot::XYZ, new_rot[0], new_rot[1], new_rot[2]).transpose();
    }

    pub fn check_point_collision(&mut self, point: Vec3) -> bool {
        let relative_pos = self.basis * (point - Vec3::from_array(self.position));
		relative_pos.x.abs() <= self.size[0] && relative_pos.y.abs() <= self.size[1] && relative_pos.z.abs() <= self.size[2]
    }
}

#[derive(Clone, Copy)]
pub enum HighPassFilter {
    Gold,
    Path {
        id: Uuid,
    },
}

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct PathCollection {
    id: Uuid,
    pub name: String,
    paths: Vec<Uuid>,
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

    pub fn paths(&self) -> &Vec<Uuid> {
        &self.paths
    }

    pub fn insert(&mut self, index: usize, path_id: Uuid) {
        self.paths.insert(index, path_id);
    }

    pub fn push(&mut self, path_id: Uuid) {
        self.paths.push(path_id);
    }

    pub fn remove(&mut self, id: Uuid) {
        if let Some(index) = self.paths.iter().position(|p| *p == id) {
            self.paths.remove(index);
        }
    }

    pub fn clear_paths(&mut self) {
        self.paths.clear();
    }
}

#[derive(Serialize, Deserialize)]
pub struct CompFile {
    version: String,
    paths: HashMap<Uuid, Path>,
    trigger_data: [[[f32; 3]; 3]; 2],
    collections: Vec<PathCollection>,
}

impl CompFile {

    // for some reason glam vectors don't deserialize correctly with serde_binary
    // so i have to convert them from and to arrays myself
    pub fn new(trigger: [BoxCollider; 2], paths: HashMap<Uuid, Path>, collections: Vec<PathCollection>) -> CompFile {

        let trigger_data = [[
                trigger[0].position,//.to_array(),
                trigger[0].rotation(),
                trigger[0].size,
            ], [
                trigger[1].position,//.to_array(),
                trigger[1].rotation(),
                trigger[1].size,
            ]
        ];

        CompFile {
            version: CURRENT_FILE_VERSION.into(),
            paths,
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

    pub fn get_paths(&self) -> HashMap<Uuid, Path> {
        self.paths.clone()
    }

    pub fn get_collections(&self) -> Vec<PathCollection> {
        self.collections.clone()
    }

    pub fn from_file(file_path: String) -> Result<CompFile, Error> {
        let file_content = fs::read(file_path)?;

        let mut head : usize = 4;

        let first_field_length = serde_binary::from_slice::<u32>(&file_content[head..(head + 4)], binary_stream::Endian::Little)? as usize;
        let first_field_name = serde_binary::from_slice::<String>(&file_content[head..(head + 4 + first_field_length)], binary_stream::Endian::Little)?;
        head += 4 + first_field_length;

        if first_field_name != "version" {
            let old_comp_file = serde_binary::from_vec::<CompFile04>(file_content, binary_stream::Endian::Little)?;
            return Ok(CompFile::from(CompFile05::from(old_comp_file)));
        }

        let file_version_len = serde_binary::from_slice::<u32>(&file_content[head..(head + 4)], binary_stream::Endian::Little)? as usize;
        let file_version = serde_binary::from_slice::<String>(&file_content[head..(head + 4 + file_version_len)], binary_stream::Endian::Little)?;

        if file_version == "0.5.1" {
            Ok(serde_binary::from_vec::<CompFile>(file_content.clone(), binary_stream::Endian::Little)?)
        }
        else if file_version == "0.5" {
            Ok(CompFile::from(serde_binary::from_vec::<CompFile05>(file_content.clone(), binary_stream::Endian::Little)?))
        }
        else {
            Err(Error::Binary{ msg: format!("Version {file_version} not compatible.") })
        }
    }

	pub fn to_file(&self, file_path: String) -> Result<(), Error> {
        let file_contents = serde_binary::to_vec(self, binary_stream::Endian::Little)?;
        Ok(fs::write(file_path, file_contents)?)

        // fs::write(
        //     file_path, serde_binary::to_vec(self, binary_stream::Endian::Little).expect("[Celestial][PathLog] Error: failed to serialize comparison file!")
        // ).expect("[Celestial][PathLog] Error: failed to write comparison file!");
    }
}

// Backwards Compatibility //
// I'm not sure how to do this properly

#[derive(Serialize, Deserialize)]
struct OldPath04 {
	id: Uuid,
	time: u64,
    nodes: Vec<[f32; 3]>,
}

#[derive(Serialize, Deserialize)]
struct PathCollection04 {
    pub id: Uuid,
    pub name: String,
    pub paths: Vec<OldPath04>,
}

#[derive(Serialize, Deserialize)]
struct CompFile04 {
    pub trigger_data: [[[f32; 3]; 3]; 2],
    pub collections: Vec<PathCollection04>,
}

impl From<PathCollection04> for PathCollection05 {
    fn from(old: PathCollection04) -> Self {
        let mut paths = Vec::<Path05>::new();
        for old_path in old.paths {
            let mut times = Vec::new();
            let mut segments = Vec::new();
            times.push(old_path.time);
            segments.push(old_path.nodes);
            paths.push(Path05 { id: old_path.id, times, segments })
        }
        PathCollection05 { id: old.id, name: old.name, paths }
    }
}

impl From<CompFile04> for CompFile05 {
    fn from(old_comp_file: CompFile04) -> Self {
        let mut collections = Vec::new();
        for old_collection in old_comp_file.collections {
            collections.push(PathCollection05::from(old_collection));
        }
        CompFile05 { version: "0.5".to_string(), trigger_data: old_comp_file.trigger_data, collections }
    }
}

#[derive(Serialize, Deserialize)]
struct Path05 {
	pub id: Uuid,
	pub times: Vec<u64>,
    pub segments: Vec<Vec<[f32; 3]>>,
}

#[derive(Serialize, Deserialize)]
struct PathCollection05 {
    pub id: Uuid,
    pub name: String,
    pub paths: Vec<Path05>,
}

#[derive(Serialize, Deserialize)]
struct CompFile05 {
    pub version: String,
    pub trigger_data: [[[f32; 3]; 3]; 2],
    pub collections: Vec<PathCollection05>,
}

impl From<CompFile05> for CompFile {
    fn from(old_comp_file: CompFile05) -> Self {
        let mut collections : Vec<PathCollection> = Vec::new();
        let mut paths : HashMap<Uuid, Path> = HashMap::new();

        for old_collection in old_comp_file.collections {
            let mut new_collection = PathCollection::new(old_collection.name);
            new_collection.id = old_collection.id;

            for old_path in old_collection.paths {
                new_collection.paths.push(old_path.id);
                paths.insert(old_path.id, Path{ id: old_path.id, times: old_path.times, segments: old_path.segments });
            }

            collections.push(new_collection);
        }

        CompFile { version: CURRENT_FILE_VERSION.into(), paths, trigger_data: old_comp_file.trigger_data, collections }
    }
}