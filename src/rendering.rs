use uuid::Uuid;

use crate::{CONFIG_STATE, PATHLOG, RENDER_UPDATES, UISTATE};
use crate::pathlog::ComparisonMode;
use crate::ui::ShapeType;
use crate::pathdata::Path;
use pintar::Pintar;

pub static RECORDING_GROUP : &str = "recording";
pub static PATHS_GROUP : &str = "paths";
pub static TRIGGERS_GROUP : &str = "triggers";
pub static TELEPORTS_GROUP : &str = "teleports";
pub static SHAPES_GROUP : &str = "custom_shapes";

#[derive(Clone, Copy)]
pub struct RenderUpdates {
    pub paths: bool,
    pub triggers: bool,
    pub teleports: bool,
    pub shapes: bool,
}

impl RenderUpdates {
    pub fn new() -> Self {
        RenderUpdates { paths: false, triggers: false, teleports: false, shapes: false }
    }

    pub fn paths() -> Self {
        RenderUpdates { paths: true, triggers: false, teleports: false, shapes: false }
    }

    pub fn triggers() -> Self {
        RenderUpdates { paths: false, triggers: true, teleports: false, shapes: false }
    }

    pub fn teleports() -> Self {
        RenderUpdates { paths: false, triggers: false, teleports: true, shapes: false }
    }

    pub fn shapes() -> Self {
        RenderUpdates { paths: false, triggers: false, teleports: false, shapes: true }
    }

    pub fn or(&mut self, other: RenderUpdates) {
        self.paths |= other.paths;
        self.triggers |= other.triggers;
        self.teleports |= other.teleports;
        self.shapes |= other.shapes;
    }
}

pub fn render_path(pintar: &mut Pintar, vertex_group: String, path: &Path, color: [f32; 4], thickness: f32) {
    for segment in path.segments() {
        if segment.len() < 2 { continue; }
        pintar.add_line(vertex_group.clone(), segment, color, thickness);
    }
}

// fn render_recording_path(pintar: &mut Pintar) {
//     for segment in path.segments() {
//         if segment.len() < 2 { continue; }
//         pintar.add_line(RECORDING_GROUP.to_string(), segment, color, thickness);
//     }
// }

pub fn render_all_paths(pintar: &mut Pintar) {
    if !RENDER_UPDATES.lock().unwrap().paths { return; }

    RENDER_UPDATES.lock().unwrap().paths = false;

    pintar.clear_vertex_group(PATHS_GROUP.to_string());

    let pathlog = PATHLOG.lock().unwrap();

    let compared_paths = pathlog.compared_paths().clone();
    let ignored_paths = pathlog.ignored_paths().clone();
    let selected_paths = pathlog.selected_paths.clone();
    let comparison = pathlog.comparison();

    drop(pathlog);

    let config = CONFIG_STATE.lock().unwrap();

    let fast_color = config.fast_color;
    let slow_color = config.slow_color;
    let gold_color = config.gold_color;
    let select_color = config.select_color;

    drop(config);

    // let mut visible_collection = PathCollection::new("Visible".to_string());
    let mut selected : Vec<Uuid> = Vec::new();
    for s in selected_paths.values() {
        selected.extend(s);
    }

    for i in 0..compared_paths.len() {
        let (path_id, position) = compared_paths[i];

        let fast = fast_color;
        let slow = slow_color;
        let lerp = |a: f32, b: f32, t: f32| -> f32 { a * (1.0-t) + b * t };
        let mut color: [f32; 4];
        let mut thick: f32;

        if position == 0 {
            color = gold_color;
            thick = 0.04;
        }
        else if comparison.len == 2 {
            color = slow_color;
            thick = 0.02;
        }
        else {
            let p = (position - 1) as f32 / (comparison.len - 2) as f32;

            color = [
                lerp(fast[0], slow[0], p),
                lerp(fast[1], slow[1], p),
                lerp(fast[2], slow[2], p),
                lerp(fast[3], slow[3], p),
            ];
            thick = 0.02;
        }

        if matches!(comparison.mode, ComparisonMode::Median) {
            for ignored_id in &ignored_paths[i] {
                let ignored_color = [color[0], color[1], color[2], color[3] * 0.5];
                render_path(pintar, PATHS_GROUP.to_string(), &PATHLOG.lock().unwrap().path(&ignored_id).unwrap(), ignored_color, thick);
            }
        }

        if selected.contains(&path_id) {
            color = select_color;
            thick = 0.04;
        }

        let pathlog = PATHLOG.lock().unwrap();
        render_path(pintar, PATHS_GROUP.to_string(), &pathlog.path(&path_id).unwrap(), color, thick);
    }
}

pub fn render_triggers(pintar: &mut Pintar) {
    let pathlog = PATHLOG.lock().unwrap();

    let checkpoint_triggers = pathlog.checkpoint_triggers.clone();
    let main_triggers = pathlog.main_triggers;

    drop(pathlog);

    let config = CONFIG_STATE.lock().unwrap();

    let checkpoint_color = config.checkpoint_color;
    let trigger_colors = config.trigger_colors;

    drop(config);

    for collider in &checkpoint_triggers {
        pintar.add_default_mesh(TRIGGERS_GROUP.to_string(), pintar::primitives::cube::new(checkpoint_color).scale(collider.size).rotate(collider.rotation()).translate(collider.position));
    }

    for i in 0..2 {
        if let Some(collider) = main_triggers[i] {
            pintar.add_default_mesh(TRIGGERS_GROUP.to_string(), pintar::primitives::cube::new(trigger_colors[i]).scale(collider.size).rotate(collider.rotation()).translate(collider.position));
        }
    }
}

pub fn render_teleports(pintar: &mut Pintar) {
    let config = CONFIG_STATE.lock().unwrap();

    let accent_colors = config.accent_colors;

    drop(config);

    let ui_state = UISTATE.lock().unwrap();

    let teleports = ui_state.extra_teleports;

    drop(ui_state);

    let lerp = |a: u8, b: u8, t: f32| -> f32 { (a as f32 * (1.0-t) + b as f32 * t) / 255. };

    for i in 0..10 {
        if let Some(teleport) = &teleports[i] {
            let pos = teleport.location;

            let p = i as f32 / 10.;
            let mut color = [
                lerp(accent_colors[0][0], accent_colors[1][0], p),
                lerp(accent_colors[0][1], accent_colors[1][1], p),
                lerp(accent_colors[0][2], accent_colors[1][2], p),
                lerp(accent_colors[0][3], accent_colors[1][3], p),
            ];

            // info!("DEBUG: color: {color:?}");
            // let mut color = trigger_colors[0];
            // color[3] = 0.5;
            // pos[1] += 1.0;
            pintar.add_default_mesh(TELEPORTS_GROUP.to_string(), pintar::primitives::cylinder::new(color).scale([0.6, 0.05, 0.6]).translate(pos));
            color[3] *= 0.25;
            pintar.add_default_mesh(TELEPORTS_GROUP.to_string(), pintar::primitives::cylinder::new(color).scale([0.5, 0.052, 0.5]).translate(pos));
        }
    }

    // if let Some(teleport) = &teleports[0] {
    //     let pos = teleport.location;
    //     let mut color = trigger_colors[0];
    //     // color[3] = 0.5;
    //     // pos[1] += 1.0;
    //     pintar.add_default_mesh(TELEPORTS_GROUP.to_string(), pintar::primitives::cylinder::new(color).scale([0.6, 0.05, 0.6]).translate(pos));
    //     color[3] *= 0.25;
    //     pintar.add_default_mesh(TELEPORTS_GROUP.to_string(), pintar::primitives::cylinder::new(color).scale([0.5, 0.052, 0.5]).translate(pos));
    // }

    // if let Some(teleport) = &teleports[1] {
    //     let pos = teleport.location;
    //     let mut color = trigger_colors[1];
    //     // color[3] = 0.5;
    //     // pos[1] += 1.0;
    //     pintar.add_default_mesh(TELEPORTS_GROUP.to_string(), pintar::primitives::cylinder::new(color).scale([0.6, 0.05, 0.6]).translate(pos));
    //     color[3] *= 0.25;
    //     pintar.add_default_mesh(TELEPORTS_GROUP.to_string(), pintar::primitives::cylinder::new(color).scale([0.5, 0.052, 0.5]).translate(pos));
    // }
}

// fn render_custom_shapes(pintar: &mut Pintar, egui: &UIState) {
pub fn render_custom_shapes(pintar: &mut Pintar) {
    let ui_state = UISTATE.lock().unwrap();

    let custom_shapes = ui_state.custom_shapes.clone();

    drop(ui_state);

    for shape in &custom_shapes {
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