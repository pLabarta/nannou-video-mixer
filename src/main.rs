mod modules;
mod save;

use modules::sources::camera::{discover_cameras, CameraWorker};
use modules::{
    base_height, body_color, has_config_row, has_output, input_count,
    input_types, kind_label, module_frame, module_number, output_type, port_color,
    push_module as modules_push_module, reaches, title_buttons_for, Connection, ModuleBody,
    ModuleKind, PlacedModule, PortType, TitleButton, CATEGORIES,
};
use nannou::image;
use nannou::prelude::*;
use nokhwa::utils::CameraInfo;
use save::{load_composition, save_composition};
use std::time::{Duration, Instant};

const IDLE_TIMEOUT: Duration = Duration::from_millis(1500);
const SAVE_STATUS_TIMEOUT: Duration = Duration::from_millis(2500);
pub const SAVE_PATH: &str = "composition.json";
const PORT_RADIUS: f32 = 10.0;
const PORT_HIT_RADIUS: f32 = 18.0;
const SIDEBAR_WIDTH: f32 = 220.0;
const SIDEBAR_HEADER_H: f32 = 64.0;
const MODULE_W: f32 = 220.0;
const MINIMIZED_H: f32 = 34.0;
const PREVIEW_H: f32 = 90.0;
const DROPDOWN_ROW_H: f32 = 26.0;
const NUMBER_DRAG_SENSITIVITY: f32 = 0.02;
const FREQ_DRAG_SENSITIVITY: f32 = 0.01;

/// A decoded video frame: width, height, RGBA bytes, and a monotonic
/// version used to tell whether it's new since the last time it was read.
pub type FrameData = (u32, u32, Vec<u8>, u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Config,
    Fullscreen,
    FullscreenOverlay,
}

/// State every module has regardless of kind: whether it's collapsed to
/// just its title bar, whether its inline thumbnail is showing, and the
/// cached texture backing that thumbnail (recreated lazily, at most one
/// new texture anywhere in the app per tick — see `update`).
pub struct ModuleChrome {
    pub minimized: bool,
    pub show_preview: bool,
    pub preview_texture: Option<wgpu::Texture>,
    pub preview_version: Option<u64>,
}

impl ModuleChrome {
    pub fn new() -> Self {
        ModuleChrome {
            minimized: false,
            show_preview: false,
            preview_texture: None,
            preview_version: None,
        }
    }
}

#[derive(Clone, Copy)]
enum Drag {
    Module { id: u64, offset: Vec2 },
    Cable { from: u64 },
    Slider { id: u64 },
    /// An open-ended drag-to-adjust value (Number's value, LFO's
    /// frequency): `last_x` is the mouse x from the previous move event,
    /// so each move applies a relative delta rather than an absolute
    /// position (unlike Slider, which is bounded 0..1).
    NumberKnob { id: u64, last_x: f32 },
}

enum SidebarRow {
    Header {
        rect: Rect,
        label: &'static str,
    },
    Module {
        row_rect: Rect,
        plus_rect: Rect,
        label: &'static str,
        kind: ModuleKind,
    },
}

pub struct Model {
    view: View,
    last_mouse_move: Instant,
    mouse_down: bool,

    pub available_cameras: Vec<CameraInfo>,
    pub next_id: u64,
    pub modules: Vec<PlacedModule>,
    pub connections: Vec<Connection>,
    pub(crate) dragging: Option<Drag>,
    pub output_id: u64,
    status: Option<(String, bool, Instant)>,
}

fn main() {
    nannou::app(model)
        .update(update)
        .event(event)
        .simple_window(view)
        .run();
}

fn model(app: &App) -> Model {
    let canvas = canvas_rect(app.window_rect());
    let mut model = Model {
        view: View::Config,
        last_mouse_move: Instant::now(),
        mouse_down: false,
        available_cameras: discover_cameras(),
        next_id: 0,
        modules: Vec::new(),
        connections: Vec::new(),
        dragging: None,
        output_id: 0,
        status: None,
    };
    model.output_id = push_module(&mut model, ModuleKind::Output, canvas.xy());
    model
}

fn update(app: &App, model: &mut Model, _update: Update) {
    if model.view == View::FullscreenOverlay
        && !model.mouse_down
        && model.dragging.is_none()
        && model.last_mouse_move.elapsed() > IDLE_TIMEOUT
    {
        model.view = View::Fullscreen;
    }

    if let Some((_, _, at)) = &model.status {
        if at.elapsed() > SAVE_STATUS_TIMEOUT {
            model.status = None;
        }
    }

    // Only keep a camera's capture thread alive while it's actually in use
    // — either its own preview is on, or it's reachable (through any chain
    // of Blends etc.) from Output. Running an unused camera is wasteful
    // and, on real hardware, can make devices contend for shared
    // USB/driver resources.
    let output_id = model.output_id;
    let needed_ids: std::collections::HashSet<u64> = model
        .modules
        .iter()
        .filter(|m| reaches(&model.connections, m.id, output_id, &mut Vec::new()))
        .map(|m| m.id)
        .collect();

    for module in &mut model.modules {
        if let ModuleBody::Camera(cam) = &mut module.body {
            let needed = cam.chrome.show_preview || needed_ids.contains(&module.id);
            if needed && cam.worker.is_none() {
                if let Some(info) = cam.selected.and_then(|i| model.available_cameras.get(i)) {
                    cam.worker = Some(CameraWorker::spawn(info.index().clone()));
                }
            } else if !needed && cam.worker.is_some() {
                cam.worker = None;
            }
        }
    }

    refresh_one_texture(app, model);
}

/// Creates at most one new GPU texture per tick, across every module's own
/// preview thumbnail plus Output's feed (Output always wins the race,
/// since it drives the actual fullscreen display regardless of whether its
/// own inline preview toggle is on). Multiple textures created within the
/// same tick is the one thing that reliably correlated with a cross-camera
/// preview glitch seen in practice, so this keeps it to one, globally.
fn refresh_one_texture(app: &App, model: &mut Model) {
    let output_id = model.output_id;
    let output_version = model
        .modules
        .iter()
        .find(|m| m.id == output_id)
        .map(|m| m.body.chrome().preview_version);

    let mut pending: Option<(u64, FrameData)> = None;

    if let Some(current_version) = output_version {
        if let Some(frame) = module_frame(&model.modules, &model.connections, output_id, &mut Vec::new()) {
            if current_version != Some(frame.3) {
                pending = Some((output_id, frame));
            }
        }
    }

    if pending.is_none() {
        for module in &model.modules {
            if !module.body.chrome().show_preview {
                continue;
            }
            if let Some(frame) = module_frame(&model.modules, &model.connections, module.id, &mut Vec::new()) {
                if module.body.chrome().preview_version != Some(frame.3) {
                    pending = Some((module.id, frame));
                    break;
                }
            }
        }
    }

    let Some((id, (width, height, rgba, version))) = pending else {
        return;
    };
    let Some(buffer) = image::RgbaImage::from_raw(width, height, rgba) else {
        return;
    };
    let texture = wgpu::Texture::from_image(app, &image::DynamicImage::ImageRgba8(buffer));
    if let Some(m) = model.modules.iter_mut().find(|m| m.id == id) {
        let chrome = m.body.chrome_mut();
        chrome.preview_texture = Some(texture);
        chrome.preview_version = Some(version);
    }
}

fn event(app: &App, model: &mut Model, event: Event) {
    if let Event::WindowEvent {
        simple: Some(window_event),
        ..
    } = event
    {
        match window_event {
            WindowEvent::MouseMoved(pos) => {
                model.last_mouse_move = Instant::now();
                if model.view == View::Fullscreen {
                    model.view = View::FullscreenOverlay;
                }
                if model.view == View::Config {
                    match model.dragging {
                        Some(Drag::Module { id, offset }) => {
                            if let Some(m) = model.modules.iter_mut().find(|m| m.id == id) {
                                m.pos = pos + offset;
                            }
                        }
                        Some(Drag::Slider { id }) => {
                            if let Some(m) = model.modules.iter_mut().find(|m| m.id == id) {
                                let chrome = m.body.chrome();
                                let body =
                                    module_body_rect(m.kind, m.pos, chrome.minimized, chrome.show_preview);
                                let config = config_rect(body);
                                if let ModuleBody::Blend(blend) = &mut m.body {
                                    blend.mix = slider_value_at(config, pos.x);
                                }
                            }
                        }
                        Some(Drag::NumberKnob { id, last_x }) => {
                            let delta = pos.x - last_x;
                            if let Some(m) = model.modules.iter_mut().find(|m| m.id == id) {
                                match &mut m.body {
                                    ModuleBody::Number(number) => {
                                        number.value += delta * NUMBER_DRAG_SENSITIVITY;
                                    }
                                    ModuleBody::Lfo(lfo) => {
                                        lfo.freq = (lfo.freq + delta * FREQ_DRAG_SENSITIVITY).max(0.0);
                                    }
                                    _ => {}
                                }
                            }
                            model.dragging = Some(Drag::NumberKnob { id, last_x: pos.x });
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::MousePressed(button) => {
                model.mouse_down = true;
                if button == MouseButton::Left {
                    handle_press(app, model);
                }
            }
            WindowEvent::MouseReleased(button) => {
                model.mouse_down = false;
                if button == MouseButton::Left {
                    handle_release(app, model);
                }
            }
            _ => {}
        }
    }
}

fn handle_press(app: &App, model: &mut Model) {
    let win = app.window_rect();
    let mouse = pt2(app.mouse.x, app.mouse.y);

    match model.view {
        View::Config => handle_press_config(model, win, mouse),
        View::FullscreenOverlay => {
            if exit_button_rect(win).contains(mouse) {
                app.main_window().set_fullscreen(false);
                model.view = View::Config;
            }
        }
        View::Fullscreen => {}
    }

    if model.view == View::Config && start_button_rect(win).contains(mouse) {
        app.main_window().set_fullscreen(true);
        model.view = View::Fullscreen;
        model.last_mouse_move = Instant::now();
    }
}

fn handle_press_config(model: &mut Model, win: Rect, mouse: Point2) {
    // A click anywhere while a dropdown is open either picks an option or
    // dismisses it; it never falls through to anything underneath.
    if let Some(open_idx) = model.modules.iter().position(|m| {
        matches!(&m.body, ModuleBody::Camera(cam) if cam.dropdown_open)
    }) {
        let available = model.available_cameras.clone();
        let module = &model.modules[open_idx];
        let chrome = module.body.chrome();
        let body = module_body_rect(module.kind, module.pos, chrome.minimized, chrome.show_preview);
        let selector = config_rect(body);

        let mut choice = None;
        for row in 0..(1 + available.len()) {
            if dropdown_option_rect(selector, row).contains(mouse) {
                choice = Some(if row == 0 { None } else { Some(row - 1) });
                break;
            }
        }

        if let ModuleBody::Camera(cam) = &mut model.modules[open_idx].body {
            cam.dropdown_open = false;
            if let Some(pick) = choice {
                cam.select(pick);
            }
        }
        return;
    }

    if save_button_rect(win).contains(mouse) {
        let (message, is_error) = match save_composition(model) {
            Ok(()) => (format!("Saved to {SAVE_PATH}"), false),
            Err(err) => (format!("Save failed: {err}"), true),
        };
        model.status = Some((message, is_error, Instant::now()));
        return;
    }

    if load_button_rect(win).contains(mouse) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Composition (*.json)", &["json"])
            .set_file_name(SAVE_PATH)
            .pick_file()
        {
            let (message, is_error) = match load_composition(model, win, &path) {
                Ok(()) => (format!("Loaded {}", path.display()), false),
                Err(err) => (format!("Load failed: {err}"), true),
            };
            model.status = Some((message, is_error, Instant::now()));
        }
        return;
    }

    for row in sidebar_rows(win) {
        if let SidebarRow::Module {
            plus_rect, kind, ..
        } = row
        {
            if plus_rect.contains(mouse) {
                spawn_module(win, model, kind);
                return;
            }
        }
    }

    let available = model.available_cameras.clone();

    for i in (0..model.modules.len()).rev() {
        let kind = model.modules[i].kind;
        let id = model.modules[i].id;
        let (minimized, show_preview) = {
            let chrome = model.modules[i].body.chrome();
            (chrome.minimized, chrome.show_preview)
        };
        let body = module_body_rect(kind, model.modules[i].pos, minimized, show_preview);

        let mut hit_button = false;
        for (slot, button) in title_buttons_for(kind).iter().enumerate() {
            if title_button_rect(body, slot).contains(mouse) {
                match button {
                    TitleButton::Delete => {
                        model.modules.remove(i);
                        model.connections.retain(|c| c.from != id && c.to != id);
                    }
                    TitleButton::Minimize => {
                        model.modules[i].body.chrome_mut().minimized = !minimized;
                    }
                    TitleButton::Preview => {
                        model.modules[i].body.chrome_mut().show_preview = !show_preview;
                    }
                }
                hit_button = true;
                break;
            }
        }
        if hit_button {
            return;
        }

        if has_config_row(kind) && !minimized {
            let config = config_rect(body);
            if config.contains(mouse) {
                match &mut model.modules[i].body {
                    ModuleBody::Camera(cam) => {
                        if !available.is_empty() {
                            cam.dropdown_open = true;
                        }
                    }
                    ModuleBody::Blend(blend) => {
                        blend.mix = slider_value_at(config, mouse.x);
                        model.dragging = Some(Drag::Slider { id });
                    }
                    ModuleBody::Number(_) | ModuleBody::Lfo(_) => {
                        model.dragging = Some(Drag::NumberKnob { id, last_x: mouse.x });
                    }
                    ModuleBody::Output(_) | ModuleBody::Mult(_) => {}
                }
                return;
            }
        }

        if has_output(kind) && mouse.distance(module_output_port(body)) <= PORT_HIT_RADIUS {
            model.dragging = Some(Drag::Cable { from: id });
            return;
        }

        let inputs = input_count(kind);
        let mut hit_input = false;
        for input_index in 0..inputs {
            if mouse.distance(module_input_port(body, input_index, inputs)) <= PORT_HIT_RADIUS {
                // Pick up whatever's already plugged in here, if anything,
                // so it can be moved to a different input or unplugged.
                if let Some(pos) = model
                    .connections
                    .iter()
                    .position(|c| c.to == id && c.to_input == input_index)
                {
                    let from = model.connections.remove(pos).from;
                    model.dragging = Some(Drag::Cable { from });
                }
                hit_input = true;
                break;
            }
        }
        if hit_input {
            return;
        }

        if body.contains(mouse) {
            model.dragging = Some(Drag::Module {
                id,
                offset: model.modules[i].pos - mouse,
            });
            return;
        }
    }
}

fn handle_release(app: &App, model: &mut Model) {
    let drag = model.dragging.take();
    let Some(Drag::Cable { from }) = drag else {
        return;
    };

    let Some(from_type) = model
        .modules
        .iter()
        .find(|m| m.id == from)
        .and_then(|m| output_type(m.kind))
    else {
        return;
    };

    let mouse = pt2(app.mouse.x, app.mouse.y);
    let mut target = None;

    for module in &model.modules {
        let types = input_types(module.kind);
        if types.is_empty() {
            continue;
        }
        let chrome = module.body.chrome();
        let body = module_body_rect(module.kind, module.pos, chrome.minimized, chrome.show_preview);
        for (input_index, port_type) in types.iter().enumerate() {
            // A cable can only land on an input of the same signal type —
            // dropping a video cable on a number input (or vice versa)
            // just fails to connect, same as dropping on empty space.
            if *port_type != from_type {
                continue;
            }
            if mouse.distance(module_input_port(body, input_index, types.len())) <= PORT_HIT_RADIUS {
                target = Some((module.id, input_index));
                break;
            }
        }
        if target.is_some() {
            break;
        }
    }

    if let Some((to, to_input)) = target {
        model.connections.retain(|c| !(c.to == to && c.to_input == to_input));
        model.connections.push(Connection { from, to, to_input });
    }
    // Dropped in empty space (or on a mismatched port type): stays
    // disconnected (already removed from `connections` in handle_press
    // if it was picked up from an input).
}

fn push_module(model: &mut Model, kind: ModuleKind, pos: Point2) -> u64 {
    modules_push_module(&mut model.modules, &mut model.next_id, kind, pos)
}

fn spawn_module(win: Rect, model: &mut Model, kind: ModuleKind) {
    let canvas = canvas_rect(win);
    let count = model.modules.len() as f32;
    let pos = canvas.xy() + vec2(30.0 * count, -30.0 * count);
    push_module(model, kind, pos);
}

// --- Layout ------------------------------------------------------------

pub fn canvas_rect(win: Rect) -> Rect {
    win.pad_left(SIDEBAR_WIDTH)
}

fn sidebar_rect(win: Rect) -> Rect {
    Rect::from_corners(
        pt2(win.left(), win.bottom()),
        pt2(win.left() + SIDEBAR_WIDTH, win.top()),
    )
}

/// The Save button pinned above the category list, spanning the sidebar.
fn save_button_rect(win: Rect) -> Rect {
    let sidebar = sidebar_rect(win);
    let w = (sidebar.w() - 16.0 - 8.0) / 2.0;
    Rect::from_x_y_w_h(sidebar.x() - w / 2.0 - 4.0, sidebar.top() - SIDEBAR_HEADER_H / 2.0, w, 32.0)
}

fn load_button_rect(win: Rect) -> Rect {
    let sidebar = sidebar_rect(win);
    let w = (sidebar.w() - 16.0 - 8.0) / 2.0;
    Rect::from_x_y_w_h(sidebar.x() + w / 2.0 + 4.0, sidebar.top() - SIDEBAR_HEADER_H / 2.0, w, 32.0)
}

fn sidebar_rows(win: Rect) -> Vec<SidebarRow> {
    let sidebar = sidebar_rect(win);
    let mut rows = Vec::new();
    let mut y = sidebar.top() - SIDEBAR_HEADER_H - 24.0;

    for (category, mods) in CATEGORIES {
        let rect = Rect::from_x_y_w_h(sidebar.x(), y, sidebar.w() - 16.0, 30.0);
        rows.push(SidebarRow::Header {
            rect,
            label: category,
        });
        y -= 38.0;

        for (label, kind) in mods.iter() {
            let row_rect = Rect::from_x_y_w_h(sidebar.x() + 6.0, y, sidebar.w() - 28.0, 32.0);
            let plus_rect = Rect::from_w_h(24.0, 24.0).mid_right_of(row_rect).shift_x(-6.0);
            rows.push(SidebarRow::Module {
                row_rect,
                plus_rect,
                label,
                kind: *kind,
            });
            y -= 38.0;
        }

        y -= 14.0;
    }

    rows
}

/// Height of a module box: collapsed to just the title bar when minimized,
/// otherwise its normal content plus a preview strip if that's enabled.
fn body_height(minimized: bool, show_preview: bool, base: f32) -> f32 {
    if minimized {
        MINIMIZED_H
    } else if show_preview {
        base + PREVIEW_H + 8.0
    } else {
        base
    }
}

fn module_body_rect(kind: ModuleKind, pos: Point2, minimized: bool, show_preview: bool) -> Rect {
    Rect::from_xy_wh(pos, vec2(MODULE_W, body_height(minimized, show_preview, base_height(kind))))
}

/// The one config-control slot beneath the title, shared by every kind
/// that has a config row (Camera's dropdown, Blend's slider, ...).
fn config_rect(body: Rect) -> Rect {
    Rect::from_x_y_w_h(body.x(), body.top() - 52.0, body.w() - 24.0, 32.0)
}

fn preview_rect(body: Rect, below_config_row: bool) -> Rect {
    let y = if below_config_row {
        body.top() - 52.0 - 16.0 - 8.0 - PREVIEW_H / 2.0
    } else {
        body.top() - 34.0 - 8.0 - PREVIEW_H / 2.0
    };
    Rect::from_x_y_w_h(body.x(), y, body.w() - 24.0, PREVIEW_H)
}

fn dropdown_option_rect(selector: Rect, row: usize) -> Rect {
    let y = selector.bottom() - DROPDOWN_ROW_H / 2.0 - row as f32 * DROPDOWN_ROW_H;
    Rect::from_x_y_w_h(selector.x(), y, selector.w(), DROPDOWN_ROW_H)
}

fn slider_value_at(rect: Rect, mouse_x: f32) -> f32 {
    ((mouse_x - rect.left()) / rect.w()).clamp(0.0, 1.0)
}

fn title_button_rect(body: Rect, slot: usize) -> Rect {
    Rect::from_w_h(22.0, 22.0)
        .top_right_of(body)
        .shift(vec2(-8.0 - slot as f32 * 26.0, -8.0))
}

fn module_output_port(body: Rect) -> Point2 {
    pt2(body.right(), body.y())
}

/// Input ports are spaced evenly down the left edge; a single input sits
/// dead center, matching every module that used to have exactly one.
fn module_input_port(body: Rect, index: usize, total: usize) -> Point2 {
    if total <= 1 {
        return pt2(body.left(), body.y());
    }
    let step = body.h() / (total as f32 + 1.0);
    pt2(body.left(), body.top() - step * (index as f32 + 1.0))
}

fn start_button_rect(win: Rect) -> Rect {
    Rect::from_w_h(220.0, 50.0)
        .mid_bottom_of(canvas_rect(win))
        .shift_y(40.0)
}

fn exit_button_rect(win: Rect) -> Rect {
    Rect::from_w_h(180.0, 44.0)
        .top_right_of(win)
        .shift(vec2(-110.0, -40.0))
}

// --- Drawing -----------------------------------------------------------

fn view(app: &App, model: &Model, render_frame: nannou::Frame) {
    let draw = app.draw();
    let win = app.window_rect();

    match model.view {
        View::Config => {
            draw.background().color(DIMGRAY);

            for module in &model.modules {
                draw_module(&draw, model, module, app.time);
            }
            draw_cables(&draw, app, model);

            draw_button(&draw, start_button_rect(win), "Start Fullscreen");
            draw_sidebar(&draw, win, model);

            // Open dropdown renders last so its options sit on top of everything.
            for module in &model.modules {
                if let ModuleBody::Camera(cam) = &module.body {
                    if cam.dropdown_open {
                        let body = module_body_rect(
                            module.kind,
                            module.pos,
                            cam.chrome.minimized,
                            cam.chrome.show_preview,
                        );
                        draw_dropdown_options(&draw, body, &model.available_cameras);
                    }
                }
            }
        }
        View::Fullscreen | View::FullscreenOverlay => {
            draw.background().color(BLACK);
            draw_output_signal(&draw, win, model);

            if model.view == View::FullscreenOverlay {
                draw_button(&draw, exit_button_rect(win), "Exit Fullscreen");
            }
        }
    }

    draw.to_frame(app, &render_frame).unwrap();
}

fn draw_sidebar(draw: &Draw, win: Rect, model: &Model) {
    let sidebar = sidebar_rect(win);
    draw.rect()
        .xy(sidebar.xy())
        .wh(sidebar.wh())
        .color(rgb(0.13, 0.13, 0.13));

    let save_rect = save_button_rect(win);
    draw_button(draw, save_rect, "Save");

    let load_rect = load_button_rect(win);
    draw_button(draw, load_rect, "Load");

    if let Some((message, is_error, _)) = &model.status {
        draw.text(message)
            .xy(pt2(sidebar.x(), save_rect.y() - 24.0))
            .wh(vec2(sidebar.w() - 16.0, 20.0))
            .color(if *is_error { CRIMSON } else { LIMEGREEN })
            .font_size(12);
    }

    for row in sidebar_rows(win) {
        match row {
            SidebarRow::Header { rect, label } => {
                draw.rect()
                    .xy(rect.xy())
                    .wh(rect.wh())
                    .color(rgb(0.22, 0.22, 0.22));
                draw.text(label).xy(rect.xy()).color(WHITE).font_size(16);
            }
            SidebarRow::Module {
                row_rect,
                plus_rect,
                label,
                ..
            } => {
                draw.text(label)
                    .xy(row_rect.xy() - vec2(14.0, 0.0))
                    .color(LIGHTGRAY)
                    .font_size(14);
                draw.rect()
                    .xy(plus_rect.xy())
                    .wh(plus_rect.wh())
                    .color(STEELBLUE);
                draw.text("+").xy(plus_rect.xy()).color(WHITE).font_size(16);
            }
        }
    }
}

fn draw_title_buttons(draw: &Draw, body: Rect, buttons: &[TitleButton]) {
    for (slot, button) in buttons.iter().enumerate() {
        let rect = title_button_rect(body, slot);
        let (color, glyph) = match button {
            TitleButton::Delete => (CRIMSON, "x"),
            TitleButton::Minimize => (STEELBLUE, "_"),
            TitleButton::Preview => (STEELBLUE, "P"),
        };
        draw.rect().xy(rect.xy()).wh(rect.wh()).color(color);
        draw.text(glyph).xy(rect.xy()).color(WHITE).font_size(13);
    }
}

fn draw_module(draw: &Draw, model: &Model, module: &PlacedModule, now: f32) {
    let chrome = module.body.chrome();
    let body = module_body_rect(module.kind, module.pos, chrome.minimized, chrome.show_preview);

    draw.rect().xy(body.xy()).wh(body.wh()).color(body_color(module.kind));
    draw.rect()
        .xy(body.xy())
        .wh(body.wh())
        .no_fill()
        .stroke(BLACK)
        .stroke_weight(2.0);

    let title_y = if chrome.minimized { body.y() } else { body.top() - 18.0 };
    draw.text(kind_label(module.kind))
        .xy(pt2(body.x(), title_y))
        .color(WHITE)
        .font_size(16);

    draw_title_buttons(draw, body, title_buttons_for(module.kind));

    if !chrome.minimized {
        if has_config_row(module.kind) {
            let config = config_rect(body);
            match &module.body {
                ModuleBody::Camera(cam) => {
                    draw.rect().xy(config.xy()).wh(config.wh()).color(SNOW);
                    draw.text(&format!("{} \u{25BE}", cam.label(&model.available_cameras)))
                        .xy(config.xy())
                        .wh(config.wh())
                        .color(BLACK)
                        .font_size(12);
                }
                ModuleBody::Blend(blend) => draw_slider(draw, config, blend.mix),
                ModuleBody::Number(number) => {
                    draw_value_box(draw, config, &format!("{:.2}", number.value));
                }
                ModuleBody::Lfo(lfo) => {
                    let value = module_number(&model.modules, &model.connections, module.id, now, &mut Vec::new())
                        .unwrap_or(0.0);
                    draw_value_box(draw, config, &format!("{:.2} Hz = {:.2}", lfo.freq, value));
                }
                ModuleBody::Mult(_) => {
                    let value = module_number(&model.modules, &model.connections, module.id, now, &mut Vec::new());
                    let text = match value {
                        Some(v) => format!("= {:.2}", v),
                        None => "= \u{2014}".to_string(),
                    };
                    draw_value_box(draw, config, &text);
                }
                ModuleBody::Output(_) => {}
            }
        }

        if chrome.show_preview {
            let preview = preview_rect(body, has_config_row(module.kind));
            draw_preview(draw, preview, chrome.preview_texture.as_ref());
        }
    }

    if let Some(port_type) = output_type(module.kind) {
        let connected = model.connections.iter().any(|c| c.from == module.id);
        draw_port(draw, module_output_port(body), port_type, connected);
    }

    let types = input_types(module.kind);
    for (i, port_type) in types.iter().enumerate() {
        let connected = model.connections.iter().any(|c| c.to == module.id && c.to_input == i);
        draw_port(draw, module_input_port(body, i, types.len()), *port_type, connected);
    }
}

/// A port is always filled with its signal-type color (video = yellow,
/// number = black) so the type is visible regardless of connection state;
/// connected ports are solid, unconnected ones are hollow rings.
fn draw_port(draw: &Draw, pos: Point2, port_type: PortType, connected: bool) {
    let color = port_color(port_type);
    if connected {
        draw.ellipse().xy(pos).radius(PORT_RADIUS).color(color);
    } else {
        draw.ellipse().xy(pos).radius(PORT_RADIUS).color(WHITE);
        draw.ellipse()
            .xy(pos)
            .radius(PORT_RADIUS)
            .no_fill()
            .stroke(color)
            .stroke_weight(2.5);
    }
}

fn draw_slider(draw: &Draw, rect: Rect, mix: f32) {
    draw.rect().xy(rect.xy()).wh(rect.wh()).color(SNOW);
    let fill_w = (rect.w() * mix).max(2.0);
    let fill = Rect::from_x_y_w_h(rect.left() + fill_w / 2.0, rect.y(), fill_w, rect.h());
    draw.rect().xy(fill.xy()).wh(fill.wh()).color(STEELBLUE);
    draw.rect()
        .xy(rect.xy())
        .wh(rect.wh())
        .no_fill()
        .stroke(rgb(0.4, 0.4, 0.4))
        .stroke_weight(1.0);
    draw.text(&format!("Mix {:.2}", mix))
        .xy(rect.xy())
        .color(BLACK)
        .font_size(12);
}

/// A plain value readout, used for Number's and LFO's drag-to-adjust
/// controls and Mult's live (read-only) result.
fn draw_value_box(draw: &Draw, rect: Rect, text: &str) {
    draw.rect().xy(rect.xy()).wh(rect.wh()).color(SNOW);
    draw.rect()
        .xy(rect.xy())
        .wh(rect.wh())
        .no_fill()
        .stroke(rgb(0.4, 0.4, 0.4))
        .stroke_weight(1.0);
    draw.text(text).xy(rect.xy()).color(BLACK).font_size(13);
}

fn draw_dropdown_options(draw: &Draw, body: Rect, available: &[CameraInfo]) {
    let selector = config_rect(body);
    let rows = 1 + available.len();

    for row in 0..rows {
        let rect = dropdown_option_rect(selector, row);
        draw.rect().xy(rect.xy()).wh(rect.wh()).color(WHITE);
        draw.rect()
            .xy(rect.xy())
            .wh(rect.wh())
            .no_fill()
            .stroke(rgb(0.6, 0.6, 0.6))
            .stroke_weight(1.0);
        let label = if row == 0 {
            "None".to_string()
        } else {
            available[row - 1].human_name().to_string()
        };
        draw.text(&label)
            .xy(rect.xy())
            .wh(rect.wh())
            .color(BLACK)
            .font_size(12);
    }
}

fn draw_preview(draw: &Draw, rect: Rect, texture: Option<&wgpu::Texture>) {
    draw.rect().xy(rect.xy()).wh(rect.wh()).color(BLACK);
    match texture {
        Some(texture) => {
            let [img_w, img_h] = texture.size();
            let scale = (rect.w() / img_w as f32).min(rect.h() / img_h as f32);
            draw.texture(texture)
                .xy(rect.xy())
                .w_h(img_w as f32 * scale, img_h as f32 * scale);
        }
        None => {
            draw.text("No Signal")
                .xy(rect.xy())
                .color(GRAY)
                .font_size(14);
        }
    }
    draw.rect()
        .xy(rect.xy())
        .wh(rect.wh())
        .no_fill()
        .stroke(rgb(0.4, 0.4, 0.4))
        .stroke_weight(1.0);
}

fn draw_cables(draw: &Draw, app: &App, model: &Model) {
    for c in &model.connections {
        let from = model.modules.iter().find(|m| m.id == c.from);
        let to = model.modules.iter().find(|m| m.id == c.to);
        if let (Some(from), Some(to)) = (from, to) {
            let from_chrome = from.body.chrome();
            let from_body =
                module_body_rect(from.kind, from.pos, from_chrome.minimized, from_chrome.show_preview);
            let to_chrome = to.body.chrome();
            let to_body = module_body_rect(to.kind, to.pos, to_chrome.minimized, to_chrome.show_preview);
            let color = output_type(from.kind).map(port_color).unwrap_or(LIGHTGRAY);

            draw.line()
                .start(module_output_port(from_body))
                .end(module_input_port(to_body, c.to_input, input_count(to.kind)))
                .weight(4.0)
                .color(color);
        }
    }

    if let Some(Drag::Cable { from }) = &model.dragging {
        if let Some(module) = model.modules.iter().find(|m| m.id == *from) {
            let chrome = module.body.chrome();
            let body = module_body_rect(module.kind, module.pos, chrome.minimized, chrome.show_preview);
            let color = output_type(module.kind).map(port_color).unwrap_or(LIGHTGRAY);
            draw.line()
                .start(module_output_port(body))
                .end(pt2(app.mouse.x, app.mouse.y))
                .weight(4.0)
                .color(color);
        }
    }
}

fn draw_button(draw: &Draw, rect: Rect, label: &str) {
    draw.rect().xy(rect.xy()).wh(rect.wh()).color(SNOW);
    draw.rect()
        .xy(rect.xy())
        .wh(rect.wh())
        .no_fill()
        .stroke(BLACK)
        .stroke_weight(2.0);
    draw.text(label).xy(rect.xy()).color(BLACK).font_size(18);
}

/// Draws whatever Output currently carries: its cached preview texture
/// (kept fresh unconditionally — see `refresh_one_texture`), or a "no
/// signal" placeholder if nothing's reached it yet.
fn draw_output_signal(draw: &Draw, win: Rect, model: &Model) {
    let texture = model
        .modules
        .iter()
        .find(|m| m.id == model.output_id)
        .and_then(|m| m.body.chrome().preview_texture.as_ref());

    match texture {
        Some(texture) => {
            let [img_w, img_h] = texture.size();
            let scale = (win.w() / img_w as f32).min(win.h() / img_h as f32);
            draw.texture(texture)
                .w_h(img_w as f32 * scale, img_h as f32 * scale);
        }
        None => {
            draw.text("No Signal").color(GRAY).font_size(32);
        }
    }
}
