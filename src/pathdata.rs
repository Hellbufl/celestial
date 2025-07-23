use std::fs;
use std::vec::Vec;
// use tracing::{error, info};
use glam::{Vec3, Mat3};
use serde_binary::binary_stream;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

use crate::error::Error;

const FILE_VERSION : &str = "0.5";

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct OldPath {
	id: Uuid,
	time: u64,
    nodes: Vec<[f32; 3]>,
}

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

    // pub fn set_time(&mut self, new_time: u64) {
    //     self.time = new_time;
    // }

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

    // pub fn clear_segment(&mut self, index: usize) {
    //     if self.segments.len() < index { return; }
    //     self.segments[index].clear();
    //     self.times[index] = 0;
    // }

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
            basis: Mat3::from_euler(glam::EulerRot::XYZ, rotation[0], rotation[1], rotation[2]).transpose(), // transposing cause xA = A^Tx and i can't do Vec3 * Mat3
            size,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    // pub fn position(&self) -> [f32; 3] {
    //     self.position.into()
    // }

    // pub fn set_position(&mut self, new_pos: [f32; 3]) {
    //     self.position =
    // }

    pub fn basis(&self) -> [[f32; 3]; 3] {
        self.basis.to_cols_array_2d()
    }

    // pub fn size(&self) -> [f32; 3] {
    //     self.size
    // }

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

pub enum HighPassFilter {
    GOLD,
    PATH {
        id: Uuid,
    },
}

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct OldPathCollection {
    pub id: Uuid,
    pub name: String,
    pub paths: Vec<OldPath>,
}

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct PathCollection {
    id: Uuid,
    pub name: String,
    paths: Vec<Path>,
}

impl From<OldPathCollection> for PathCollection {
    fn from(old: OldPathCollection) -> Self {
        let mut paths = Vec::<Path>::new();
        for old_path in old.paths {
            let mut times = Vec::new();
            let mut segments = Vec::new();
            times.push(old_path.time);
            segments.push(old_path.nodes);
            paths.push(Path { id: old_path.id, times, segments })
        }
        PathCollection { id: old.id, name: old.name, paths }
    }
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
pub struct OldCompFile {
    pub trigger_data: [[[f32; 3]; 3]; 2],
    pub collections: Vec<OldPathCollection>,
}

#[derive(Serialize, Deserialize)]
pub struct CompFile {
    version: String,
    trigger_data: [[[f32; 3]; 3]; 2],
    collections: Vec<PathCollection>,
}

impl CompFile {

    // for some reason glam vectors don't deserialize correctly with serde_binary
    // so i have to convert them from and to arrays myself
    pub fn new(trigger: [BoxCollider; 2], collections: Vec<PathCollection>) -> CompFile {

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
            version: FILE_VERSION.into(),
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

    pub fn from_file(file_path: String) -> Result<CompFile, Error> {
        let file_content = fs::read(file_path)?;

        let version_bytes = FILE_VERSION.as_bytes().to_vec();
        let header_bytes = vec![vec![3, 0, 0, 0, 7, 0, 0, 0], "version".as_bytes().to_vec(), (version_bytes.len() as u32).to_le_bytes().to_vec(), version_bytes].concat();

        if file_content[..header_bytes.len() + 1].iter().zip(&header_bytes).filter(|&(a, b)| a == b).count() == header_bytes.len() {
            Ok(serde_binary::from_vec::<CompFile>(file_content.clone(), binary_stream::Endian::Little)?)
        }
        else {
            let old_comp_file = serde_binary::from_vec::<OldCompFile>(file_content, binary_stream::Endian::Little)?;
            let mut collections = Vec::new();
            for old_collection in old_comp_file.collections {
                collections.push(PathCollection::from(old_collection));
            }
            Ok(CompFile { version: FILE_VERSION.into(), trigger_data: old_comp_file.trigger_data, collections })
        }
    }

	pub fn to_file(&self, file_path: String) {
        fs::write(
            file_path, serde_binary::to_vec(self, binary_stream::Endian::Little).expect("[Celestial][PathLog] Error: failed to serialize comparison file!")
        ).expect("[Celestial][PathLog] Error: failed to write comparison file!");
    }
}