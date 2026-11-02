//! Waypoints: named, coloured places the player saves. Each world has its own list, each
//! dimension shows only its own, and the last three deaths of a dimension are kept
//! automatically. They are drawn in the world (name and distance) at the player's opacity and
//! managed with chat commands: `!wp` lists them, `!wp help` says how.
use super::{Dim, Game};
use horizon_client::{
    hud::{HudBatch, rgba},
    text::Text,
};
use std::cell::RefCell;
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
