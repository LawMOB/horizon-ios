//! Waypoints: named, coloured places the player saves. Each world has its own list, each
//! dimension shows only its own, and the last three deaths of a dimension are kept
//! automatically. They are drawn in the world (name and distance) at the player's opacity and
//! managed with chat commands: `!wp` lists them, `!wp help` says how.
use super::{Dim, Game};
use horizon_client::{
    hud::{HudBatch, rgba},
    text::Text,
    touch::Rect,
};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;

/// Deaths kept per dimension.
const MAX_DEATHS: usize = 3;
/// Longest waypoint name, in characters.
const MAX_NAME: usize = 24;

/// The colours a waypoint can have: name and RGB.
const COLORS: [(&str, [u8; 3]); 8] = [
    ("rojo", [235, 70, 70]),
    ("naranja", [255, 150, 50]),
    ("amarillo", [250, 220, 70]),
    ("verde", [90, 220, 100]),
    ("celeste", [90, 210, 240]),
    ("azul", [90, 130, 255]),
    ("morado", [190, 120, 255]),
    ("blanco", [240, 240, 240]),
];

#[derive(Clone, Debug, PartialEq)]
struct Waypoint {
    death: bool,
    dim: String,
    pos: [i32; 3],
    color: usize,
    name: String,
}

#[derive(Default)]
struct Store {
    /// The world these belong to (file name), empty before any is loaded.
    world: String,
    list: Vec<Waypoint>,
    /// 0..=1, how solid the markers are.
    opacity: f32,
    opacity_loaded: bool,
}

thread_local! {
    static STORE: RefCell<Store> = RefCell::new(Store::default());
    /// The waypoints panel (the HUD button) is open, and on which page of the list.
    static PANEL: Cell<bool> = const { Cell::new(false) };
    static PAGE: Cell<usize> = const { Cell::new(0) };
}

/// The panel is open: it takes every touch.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub(super) fn panel_is_open() -> bool {
    PANEL.get()
}

/// Where the panel's parts are on a screen of `w` x `h` GUI pixels.
struct PanelRects {
    body: Rect,
    rows: Vec<Rect>,
    prev: Rect,
    next: Rect,
    add: Rect,
    less: Rect,
    value: Rect,
    more: Rect,
    close: Rect,
}

const ROW: f32 = 17.0;

fn panel_rects(w: f32, h: f32) -> PanelRects {
    let n = (((h - 62.0) / ROW) as usize).clamp(2, 5);
    let width = (w - 16.0).clamp(160.0, 300.0);
    let height = 18.0 + n as f32 * ROW + 24.0;
    let x = ((w - width) / 2.0).round();
    let y = ((h - height) / 2.0).max(2.0).round();
    let rows = (0..n)
        .map(|i| Rect::new(x + 4.0, y + 18.0 + i as f32 * ROW, width - 8.0, ROW - 1.0))
        .collect();
    let foot = y + 18.0 + n as f32 * ROW + 3.0;
    let unit = (width - 8.0) / 8.0;
    PanelRects {
        body: Rect::new(x, y, width, height),
        rows,
        prev: Rect::new(x + width - 44.0, y + 3.0, 19.0, 13.0),
        next: Rect::new(x + width - 23.0, y + 3.0, 19.0, 13.0),
        add: Rect::new(x + 4.0, foot, unit * 2.5 - 2.0, 17.0),
        less: Rect::new(x + 4.0 + unit * 2.5, foot, unit - 2.0, 17.0),
        value: Rect::new(x + 4.0 + unit * 3.5, foot, unit * 1.5 - 2.0, 17.0),
        more: Rect::new(x + 4.0 + unit * 5.0, foot, unit - 2.0, 17.0),
        close: Rect::new(x + 4.0 + unit * 6.0, foot, unit * 2.0, 17.0),
    }
}

/// A row's parts: the colour square, the name, the distance and the delete button.
fn row_parts(r: Rect) -> (Rect, Rect, Rect) {
    (
        Rect::new(r.x, r.y, 16.0, r.h),
        Rect::new(r.x + 18.0, r.y, r.w - 18.0 - 52.0 - 18.0, r.h),
        Rect::new(r.x + r.w - 16.0, r.y, 16.0, r.h),
    )
}

fn button(batch: &mut HudBatch, atlas: &horizon_client::hud::HudAtlas, r: Rect, label: &str) {
    batch.fill(atlas, r.x, r.y, r.w, r.h, rgba(90, 90, 90, 230));
    batch.fill(
        atlas,
        r.x + 1.0,
        r.y + 1.0,
        r.w - 2.0,
        r.h - 2.0,
        rgba(50, 50, 50, 230),
    );
    let tw = HudBatch::text_width(atlas, label);
    batch.text(
        atlas,
        label,
        (r.x + (r.w - tw) / 2.0).round(),
        (r.y + (r.h - 8.0) / 2.0).round(),
        rgba(240, 240, 240, 255),
        true,
    );
}

fn dir() -> PathBuf {
    Game::options_path().with_file_name("waypoints")
}

/// A world's name as a safe file name.
fn file_key(name: &str) -> String {
    let key: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if key.is_empty() {
        "world".to_owned()
    } else {
        key
    }
}

fn parse_line(line: &str) -> Option<Waypoint> {
    let mut it = line.splitn(7, '|');
    let death = match it.next()? {
        "d" => true,
        "w" => false,
        _ => return None,
    };
    let dim = it.next()?.to_owned();
    let x = it.next()?.parse().ok()?;
    let y = it.next()?.parse().ok()?;
    let z = it.next()?.parse().ok()?;
    let color = it.next()?.parse::<usize>().ok()?.min(COLORS.len() - 1);
    let name = it.next().unwrap_or("").to_owned();
    Some(Waypoint {
        death,
        dim,
        pos: [x, y, z],
        color,
        name,
    })
}

fn format_line(w: &Waypoint) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}",
        if w.death { "d" } else { "w" },
        w.dim,
        w.pos[0],
        w.pos[1],
        w.pos[2],
        w.color,
        w.name
    )
}

impl Store {
    fn load_opacity(&mut self) {
        if self.opacity_loaded {
            return;
        }
        self.opacity_loaded = true;
        self.opacity = std::fs::read_to_string(dir().join("_settings.txt"))
            .ok()
            .and_then(|t| {
                t.lines()
                    .find_map(|l| l.strip_prefix("opacity="))
                    .and_then(|v| v.trim().parse::<f32>().ok())
            })
            .unwrap_or(1.0)
            .clamp(0.1, 1.0);
    }

    fn save_opacity(&self) {
        let _ = std::fs::create_dir_all(dir());
        let _ = std::fs::write(
            dir().join("_settings.txt"),
            format!("opacity={:.2}\n", self.opacity),
        );
    }

    /// Switches to `world`'s list, reading its file when it is another world than the last.
    fn open(&mut self, world: &str) {
        self.load_opacity();
        let key = file_key(world);
        if self.world == key {
            return;
        }
        self.world = key.clone();
        self.list = std::fs::read_to_string(dir().join(format!("{key}.txt")))
            .map(|t| t.lines().filter_map(parse_line).collect())
            .unwrap_or_default();
    }

    fn save(&self) {
        if self.world.is_empty() {
            return;
        }
        let _ = std::fs::create_dir_all(dir());
        let text: String = self.list.iter().map(|w| format_line(w) + "\n").collect();
        let _ = std::fs::write(dir().join(format!("{}.txt", self.world)), text);
    }

    /// The waypoints of one dimension, as indices into `list` (the numbers the player sees count
    /// from 1 in this order).
    fn of(&self, dim: &str) -> Vec<usize> {
        (0..self.list.len())
            .filter(|&i| self.list[i].dim == dim)
            .collect()
    }

    fn add_death(&mut self, dim: &str, pos: [i32; 3]) {
        self.list.push(Waypoint {
            death: true,
            dim: dim.to_owned(),
            pos,
            color: 0,
            name: "Muerte".to_owned(),
        });
        // Keep the newest MAX_DEATHS of this dimension.
        let deaths: Vec<usize> = (0..self.list.len())
            .filter(|&i| self.list[i].death && self.list[i].dim == dim)
            .collect();
        let excess = deaths.len().saturating_sub(MAX_DEATHS);
        for &i in deaths[..excess].iter().rev() {
            self.list.remove(i);
        }
    }
}

fn color_of(word: &str) -> Option<usize> {
    if let Ok(n) = word.parse::<usize>() {
        return (1..=COLORS.len()).contains(&n).then(|| n - 1);
    }
    COLORS
        .iter()
        .position(|(name, _)| *name == word.to_lowercase())
}

fn clean_name(name: &str) -> String {
    name.trim()
        .replace('|', "/")
        .chars()
        .take(MAX_NAME)
        .collect()
}

fn dim_title(dim: &str) -> &'static str {
    match dim {
        "minecraft:the_nether" => "Nether",
        "minecraft:the_end" => "End",
        _ => "Overworld",
    }
}

impl Game<'_> {
    /// The current world's name, `None` on the title screen.
    fn waypoint_world(&self) -> Option<String> {
        if self.title.is_some() || self.player.is_none() {
            return None;
        }
        Some(match &self.integrated {
            Some(server) if !server.world_name.is_empty() => server.world_name.clone(),
            _ => self.level.name.clone(),
        })
    }

    /// Player.die: the place of a death becomes a red waypoint (the last three per dimension).
    pub(super) fn waypoint_death(&mut self, dim: Dim, feet: [f64; 3]) {
        let Some(world) = self.waypoint_world() else {
            return;
        };
        STORE.with_borrow_mut(|s| {
            s.open(&world);
            s.add_death(dim.id(), feet.map(|v| v.floor() as i32));
            s.save();
        });
    }

    /// A chat line that starts with `!wp`: the waypoint commands. Returns whether it was one.
    pub(super) fn waypoint_command(&mut self, message: &str) -> bool {
        let Some(rest) = message.strip_prefix("!wp") else {
            return false;
        };
        if !rest.is_empty() && !rest.starts_with(' ') {
            return false;
        }
        let Some(world) = self.waypoint_world() else {
            return true;
        };
        let dim = self.dim.id();
        let here = self
            .player
            .as_ref()
            .map(|p| [p.position.x, p.position.y, p.position.z].map(|v| v.floor() as i32));
        let rest = rest.trim();
        let (cmd, args) = rest.split_once(' ').unwrap_or((rest, ""));
        let args = args.trim();
        let mut out: Vec<String> = Vec::new();
        STORE.with_borrow_mut(|s| {
            s.open(&world);
            let mine = s.of(dim);
            // "3" in a command is the third waypoint of this dimension.
            let pick = |word: &str| -> Option<usize> {
                let n: usize = word.parse().ok()?;
                mine.get(n.checked_sub(1)?).copied()
            };
            match cmd {
                "" | "lista" | "list" => {
                    if mine.is_empty() {
                        out.push(format!(
                            "Sin waypoints en {}. Usa: !wp nuevo Casa",
                            dim_title(dim)
                        ));
                    }
                    for (n, &i) in mine.iter().enumerate() {
                        let w = &s.list[i];
                        out.push(format!(
                            "{}. {} ({}) {} {} {}",
                            n + 1,
                            w.name,
                            COLORS[w.color].0,
                            w.pos[0],
                            w.pos[1],
                            w.pos[2]
                        ));
                    }
                }
                "nuevo" | "add" | "new" => {
                    if let Some(pos) = here {
                        let color = s.list.iter().filter(|w| !w.death && w.dim == dim).count()
                            % COLORS.len();
                        // The first colour is the deaths' red.
                        let color = if color == 0 { 3 } else { color };
                        let name = if args.is_empty() {
                            format!("Punto {}", mine.len() + 1)
                        } else {
                            clean_name(args)
                        };
                        s.list.push(Waypoint {
                            death: false,
                            dim: dim.to_owned(),
                            pos,
                            color,
                            name: name.clone(),
                        });
                        s.save();
                        out.push(format!("Waypoint guardado: {name}"));
                    }
                }
                "nombre" | "name" | "renombrar" => {
                    let (n, name) = args.split_once(' ').unwrap_or((args, ""));
                    match pick(n) {
                        Some(i) if !name.trim().is_empty() => {
                            s.list[i].name = clean_name(name);
                            s.save();
                            out.push("Nombre cambiado".to_owned());
                        }
                        _ => out.push("Uso: !wp nombre 2 Mina".to_owned()),
                    }
                }
                "color" => {
                    let (n, c) = args.split_once(' ').unwrap_or((args, ""));
                    match (pick(n), color_of(c.trim())) {
                        (Some(i), Some(color)) => {
                            s.list[i].color = color;
                            s.save();
                            out.push("Color cambiado".to_owned());
                        }
                        _ => {
                            let names: Vec<&str> = COLORS.iter().map(|c| c.0).collect();
                            out.push(format!("Uso: !wp color 2 azul ({})", names.join(", ")));
                        }
                    }
                }
                "borrar" | "del" | "delete" => match pick(args) {
                    Some(i) => {
                        s.list.remove(i);
                        s.save();
                        out.push("Waypoint borrado".to_owned());
                    }
                    None => out.push("Uso: !wp borrar 2".to_owned()),
                },
                "opacidad" | "opacity" => match args.trim_end_matches('%').parse::<f32>() {
                    Ok(v) => {
                        s.opacity = (v / 100.0).clamp(0.1, 1.0);
                        s.save_opacity();
                        out.push(format!("Opacidad: {}%", (s.opacity * 100.0).round() as i32));
                    }
                    Err(_) => out.push(format!(
                        "Opacidad {}%. Uso: !wp opacidad 60",
                        (s.opacity * 100.0).round() as i32
                    )),
                },
                _ => {
                    out.push("!wp = lista | !wp nuevo [nombre] | !wp nombre N texto".to_owned());
                    out.push(
                        "!wp color N rojo..blanco | !wp borrar N | !wp opacidad 10-100".to_owned(),
                    );
                }
            }
        });
        for line in out {
            self.system_message(Text::literal(&line));
        }
        true
    }

    /// The HUD button: opens the waypoints panel.
    #[cfg_attr(not(target_os = "ios"), allow(dead_code))]
    pub(super) fn waypoint_panel_open(&mut self) {
        if self.waypoint_world().is_some() {
            PAGE.set(0);
            PANEL.set(true);
        }
    }

    /// A finger down while the panel is open.
    #[cfg_attr(not(target_os = "ios"), allow(dead_code))]
    pub(super) fn waypoint_panel_touch(&mut self, at: [f32; 2], w: f32, h: f32) {
        let Some(world) = self.waypoint_world() else {
            PANEL.set(false);
            return;
        };
        let p = panel_rects(w, h);
        if !p.body.contains(at) {
            PANEL.set(false);
            return;
        }
        let dim = self.dim.id();
        let here = self
            .player
            .as_ref()
            .map(|q| [q.position.x, q.position.y, q.position.z].map(|v| v.floor() as i32));
        let per = p.rows.len();
        let mut rename: Option<usize> = None;
        STORE.with_borrow_mut(|s| {
            s.open(&world);
            let mine = s.of(dim);
            let pages = mine.len().max(1).div_ceil(per);
            let page = PAGE.get().min(pages - 1);
            if p.close.contains(at) {
                PANEL.set(false);
            } else if p.prev.contains(at) {
                PAGE.set(page.saturating_sub(1));
            } else if p.next.contains(at) {
                PAGE.set((page + 1).min(pages - 1));
            } else if p.less.contains(at) || p.more.contains(at) {
                let step = if p.more.contains(at) { 0.1 } else { -0.1 };
                s.opacity = (((s.opacity + step) * 10.0).round() / 10.0).clamp(0.1, 1.0);
                s.save_opacity();
            } else if p.add.contains(at) {
                if let Some(pos) = here {
                    let color = 3 + mine.iter().filter(|&&i| !s.list[i].death).count() % 7;
                    let color = if color > 7 { color - 7 } else { color };
                    s.list.push(Waypoint {
                        death: false,
                        dim: dim.to_owned(),
                        pos,
                        color,
                        name: format!("Punto {}", mine.len() + 1),
                    });
                    s.save();
                    // Show the page with the new one.
                    PAGE.set(pages.max((mine.len() + 1).div_ceil(per)) - 1);
                }
            } else {
                for (n, row) in p.rows.iter().enumerate() {
                    let Some(&i) = mine.get(page * per + n) else {
                        break;
                    };
                    let (swatch, name, delete) = row_parts(*row);
                    if swatch.contains(at) {
                        s.list[i].color = (s.list[i].color + 1) % COLORS.len();
                        s.save();
                    } else if delete.contains(at) {
                        s.list.remove(i);
                        s.save();
                    } else if name.contains(at) {
                        rename = Some(page * per + n + 1);
                    }
                }
            }
        });
        // The name is typed in the chat box, which the system keyboard serves.
        if let Some(n) = rename {
            PANEL.set(false);
            self.open_chat(&format!("!wp nombre {n} "));
        }
    }

    /// The waypoints panel over a dimmed screen: this dimension's list a page at a time.
    pub(super) fn draw_waypoint_panel(&self, batch: &mut HudBatch) {
        if !PANEL.get() {
            return;
        }
        let Some(world) = self.waypoint_world() else {
            PANEL.set(false);
            return;
        };
        let atlas = &self.gui.atlas;
        let p = panel_rects(batch.width, batch.height);
        let dim = self.dim.id();
        let here = self
            .player
            .as_ref()
            .map(|q| [q.position.x, q.position.y, q.position.z]);
        let (list, opacity) = STORE.with_borrow_mut(|s| {
            s.open(&world);
            (
                s.of(dim)
                    .into_iter()
                    .map(|i| s.list[i].clone())
                    .collect::<Vec<_>>(),
                s.opacity,
            )
        });
        let per = p.rows.len();
        let pages = list.len().max(1).div_ceil(per);
        let page = PAGE.get().min(pages - 1);
        batch.fill(
            atlas,
            0.0,
            0.0,
            batch.width,
            batch.height,
            rgba(0, 0, 0, 140),
        );
        let b = p.body;
        batch.fill(atlas, b.x, b.y, b.w, b.h, rgba(110, 110, 110, 240));
        batch.fill(
            atlas,
            b.x + 1.0,
            b.y + 1.0,
            b.w - 2.0,
            b.h - 2.0,
            rgba(28, 28, 28, 245),
        );
        batch.text(
            atlas,
            &format!("Waypoints - {} ({}/{})", dim_title(dim), page + 1, pages),
            b.x + 5.0,
            b.y + 5.0,
            rgba(240, 240, 240, 255),
            true,
        );
        button(batch, atlas, p.prev, "<");
        button(batch, atlas, p.next, ">");
        if list.is_empty() {
            batch.text(
                atlas,
                "Nada aqui. Toca + Nuevo.",
                b.x + 8.0,
                p.rows[0].y + 4.0,
                rgba(170, 170, 170, 255),
                true,
            );
        }
        for (n, row) in p.rows.iter().enumerate() {
            let Some(w) = list.get(page * per + n) else {
                break;
            };
            let (swatch, name, delete) = row_parts(*row);
            batch.fill(atlas, row.x, row.y, row.w, row.h, rgba(60, 60, 60, 200));
            let [r, g, bl] = COLORS[w.color].1;
            batch.fill(
                atlas,
                swatch.x + 2.0,
                swatch.y + 2.0,
                swatch.w - 4.0,
                swatch.h - 4.0,
                rgba(r, g, bl, 255),
            );
            let fit = ((name.w / 6.0) as usize).max(4);
            let shown: String = w.name.chars().take(fit).collect();
            batch.text(
                atlas,
                &shown,
                name.x + 2.0,
                (name.y + (name.h - 8.0) / 2.0).round(),
                rgba(r, g, bl, 255),
                true,
            );
            let dist = here.map_or(String::new(), |q| {
                let d = [
                    f64::from(w.pos[0]) + 0.5 - q[0],
                    f64::from(w.pos[1]) - q[1],
                    f64::from(w.pos[2]) + 0.5 - q[2],
                ];
                let m = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                if m >= 1000.0 {
                    format!("{:.1} km", m / 1000.0)
                } else {
                    format!("{} m", m.round() as i32)
                }
            });
            batch.text(
                atlas,
                &dist,
                name.x + name.w + 2.0,
                (name.y + (name.h - 8.0) / 2.0).round(),
                rgba(200, 200, 200, 255),
                true,
            );
            button(batch, atlas, delete, "x");
        }
        button(batch, atlas, p.add, "+ Nuevo");
        button(batch, atlas, p.less, "-");
        button(
            batch,
            atlas,
            p.value,
            &format!("{}%", (opacity * 100.0).round() as i32),
        );
        button(batch, atlas, p.more, "+");
        button(batch, atlas, p.close, "Cerrar");
    }

    /// The markers of this dimension's waypoints over the world: a diamond, the name and the
    /// distance, projected from the camera. Not drawn over menus.
    pub(super) fn draw_waypoints(&self, batch: &mut HudBatch) {
        if self.ui.hide_gui || self.pad_in_menu() || self.dialog_open() {
            return;
        }
        let (Some(world), Some(eye)) = (self.waypoint_world(), self.eye_position()) else {
            return;
        };
        let dim = self.dim.id();
        let fov = f64::from(self.fov_degrees(1.0, eye)).to_radians();
        let (yaw, pitch) = (
            f64::from(self.yaw).to_radians(),
            f64::from(self.pitch).to_radians(),
        );
        let forward = [
            -yaw.sin() * pitch.cos(),
            -pitch.sin(),
            yaw.cos() * pitch.cos(),
        ];
        let right = [-yaw.cos(), 0.0, -yaw.sin()];
        // up = right x forward
        let up = [
            right[1] * forward[2] - right[2] * forward[1],
            right[2] * forward[0] - right[0] * forward[2],
            right[0] * forward[1] - right[1] * forward[0],
        ];
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let tan = (fov / 2.0).tan();
        let aspect = f64::from(batch.width / batch.height.max(1.0));
        let atlas = &self.gui.atlas;
        let (list, opacity) = STORE.with_borrow_mut(|s| {
            s.open(&world);
            (
                s.of(dim)
                    .into_iter()
                    .map(|i| s.list[i].clone())
                    .collect::<Vec<_>>(),
                s.opacity,
            )
        });
        let alpha = (255.0 * opacity).round().clamp(0.0, 255.0) as u8;
        for w in &list {
            let d = [
                f64::from(w.pos[0]) + 0.5 - eye[0],
                f64::from(w.pos[1]) + 0.5 - eye[1],
                f64::from(w.pos[2]) + 0.5 - eye[2],
            ];
            let depth = dot(d, forward);
            if depth <= 0.05 {
                continue;
            }
            let nx = dot(d, right) / depth / (tan * aspect);
            let ny = dot(d, up) / depth / tan;
            if nx.abs() > 1.0 || ny.abs() > 1.0 {
                continue;
            }
            let x = ((nx as f32 + 1.0) / 2.0 * batch.width).round();
            let y = ((1.0 - ny as f32) / 2.0 * batch.height).round();
            let [r, g, b] = COLORS[w.color].1;
            let meters = dot(d, d).sqrt();
            let dist = if meters >= 1000.0 {
                format!("{:.1} km", meters / 1000.0)
            } else {
                format!("{} m", meters.round() as i32)
            };
            batch.fill(atlas, x - 2.0, y - 2.0, 5.0, 5.0, rgba(r, g, b, alpha));
            batch.fill(atlas, x - 1.0, y - 1.0, 3.0, 3.0, rgba(0, 0, 0, alpha / 2));
            for (n, line) in [w.name.as_str(), dist.as_str()].iter().enumerate() {
                let tw = HudBatch::text_width(atlas, line);
                batch.text(
                    atlas,
                    line,
                    (x - tw / 2.0).round(),
                    y + 5.0 + n as f32 * 9.0,
                    rgba(r, g, b, alpha),
                    true,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_fits_small_and_big_screens() {
        for (w, h) in [(320.0, 240.0), (426.0, 196.0), (640.0, 295.0)] {
            let p = panel_rects(w, h);
            assert!(p.body.x >= 0.0 && p.body.x + p.body.w <= w);
            assert!(p.body.y >= 0.0 && p.body.y + p.body.h <= h, "{w}x{h}");
            assert!(p.close.x + p.close.w <= p.body.x + p.body.w);
            assert!(p.rows.len() >= 2);
        }
    }

    #[test]
    fn lines_round_trip() {
        let w = Waypoint {
            death: false,
            dim: "minecraft:the_nether".into(),
            pos: [-5, 64, 300],
            color: 4,
            name: "Mi casa".into(),
        };
        assert_eq!(parse_line(&format_line(&w)), Some(w));
    }

    #[test]
    fn only_the_last_three_deaths_stay() {
        let mut s = Store::default();
        for i in 0..5 {
            s.add_death("minecraft:overworld", [i, 0, 0]);
        }
        s.add_death("minecraft:the_end", [9, 0, 0]);
        let over: Vec<i32> = s
            .list
            .iter()
            .filter(|w| w.dim == "minecraft:overworld")
            .map(|w| w.pos[0])
            .collect();
        assert_eq!(over, vec![2, 3, 4]);
        assert_eq!(s.of("minecraft:the_end").len(), 1);
    }

    #[test]
    fn colours_by_name_or_number() {
        assert_eq!(color_of("azul"), Some(5));
        assert_eq!(color_of("1"), Some(0));
        assert_eq!(color_of("9"), None);
    }
}
