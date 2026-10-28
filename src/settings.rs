//! Settings remembered between sessions: `key<tab>value` lines in settings.tsv, in the
//! settings folder. Unknown keys are ignored, so older and newer versions share the file.

use crate::colour::{ColourBy, Colormap, Style};
use crate::common::{settings_dir, View};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;

pub fn settings_path() -> PathBuf {
    settings_dir().join("settings.tsv")
}

#[derive(Clone, Default, PartialEq, Debug)]
pub struct Settings(BTreeMap<String, String>);

impl Settings {
    pub fn from_text(text: &str) -> Settings {
        Settings(
            text.lines()
                .filter(|l| !l.starts_with('#'))
                .filter_map(|l| l.split_once('\t'))
                .map(|(k, v)| (k.trim().to_string(), v.to_string()))
                .collect(),
        )
    }

    pub fn to_text(&self) -> String {
        let mut s = String::from("# Algebraic Fractals remembered settings\n");
        for (k, v) in &self.0 {
            s += &format!("{k}\t{v}\n");
        }
        s
    }

    pub fn load() -> Settings {
        std::fs::read_to_string(settings_path()).map_or_else(|_| Settings::default(), |t| Settings::from_text(&t))
    }

    pub fn store(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(settings_dir())?;
        std::fs::write(settings_path(), self.to_text())
    }

    pub fn set(&mut self, key: &str, value: impl ToString) {
        let v: String = value.to_string().chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
        self.0.insert(key.to_string(), v);
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    /// The value, if it's there and reads as a `T`.
    pub fn get<T: FromStr>(&self, key: &str) -> Option<T> {
        self.str(key)?.trim().parse().ok()
    }

    pub fn flag(&self, key: &str) -> Option<bool> {
        self.str(key).map(|v| v.trim() == "1")
    }

    pub fn set_flag(&mut self, key: &str, v: bool) {
        self.set(key, v as u8);
    }

    pub fn set_view(&mut self, key: &str, v: View) {
        self.set(key, format!("{},{},{}", v.cr, v.ci, v.hw));
    }

    pub fn view(&self, key: &str) -> Option<View> {
        let p: Vec<f64> = self.str(key)?.split(',').map(|x| x.trim().parse::<f64>().ok().filter(|v| v.is_finite())).collect::<Option<_>>()?;
        match p[..] {
            [cr, ci, hw] if hw > 0.0 => Some(View { cr, ci, hw }),
            _ => None,
        }
    }

    /// A style, under keys starting with `prefix`.
    pub fn set_style(&mut self, prefix: &str, s: &Style) {
        self.set(&format!("{prefix}by"), s.by.short());
        self.set(&format!("{prefix}cmap"), s.cmap.name());
        for (k, v) in [
            ("n", s.n),
            ("high", s.high),
            ("low", s.low),
            ("gamma", s.gamma),
            ("cycles", s.cycles),
            ("shift", s.shift),
            ("brightness", s.brightness),
            ("contrast", s.contrast),
            ("saturation", s.saturation),
        ] {
            self.set(&format!("{prefix}{k}"), v);
        }
        for (k, v) in [("invert", s.invert), ("shade", s.shade), ("log_poly", s.log_poly), ("smooth", s.smooth)] {
            self.set_flag(&format!("{prefix}{k}"), v);
        }
    }

    /// The style under `prefix`, each missing part the default's.
    pub fn style(&self, prefix: &str) -> Style {
        let d = Style::default();
        let k = |name: &str| format!("{prefix}{name}");
        let num = |name: &str, d: f64| self.get::<f64>(&k(name)).filter(|v| v.is_finite()).unwrap_or(d);
        let flag = |name: &str, d: bool| self.flag(&k(name)).unwrap_or(d);
        Style {
            by: self.str(&k("by")).and_then(|t| ColourBy::ALL.into_iter().find(|b| b.short() == t)).unwrap_or(d.by),
            cmap: self.str(&k("cmap")).and_then(|t| Colormap::ALL.into_iter().find(|c| c.name() == t)).unwrap_or(d.cmap),
            n: num("n", d.n).clamp(0.05, 5.0),
            high: num("high", d.high).clamp(90.0, 100.0),
            low: num("low", d.low).clamp(0.0, 100.0),
            gamma: num("gamma", d.gamma).clamp(0.1, 10.0),
            cycles: num("cycles", d.cycles).clamp(1.0, 50.0),
            shift: num("shift", d.shift).clamp(0.0, 1.0),
            brightness: num("brightness", d.brightness).clamp(-0.5, 0.5),
            contrast: num("contrast", d.contrast).clamp(0.0, 4.0),
            saturation: num("saturation", d.saturation).clamp(0.0, 3.0),
            invert: flag("invert", d.invert),
            shade: flag("shade", d.shade),
            log_poly: flag("log_poly", d.log_poly),
            smooth: flag("smooth", d.smooth),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let mut s = Settings::default();
        s.set("order", 5);
        s.set("folder", "C:\\Users\\x\\Pictures\tAlgebraic");
        s.set_flag("auto_max", true);
        let v = View { cr: -0.1, ci: 1.0 / 3.0, hw: 2.5e-9 };
        s.set_view("view", v);
        let style = Style { by: ColourBy::Height, cmap: Colormap::Magma, n: 2.5, gamma: 1.7, invert: true, smooth: false, ..Style::default() };
        s.set_style("explore.", &style);
        let back = Settings::from_text(&s.to_text());
        assert_eq!(back, s);
        assert_eq!((back.get::<usize>("order"), back.flag("auto_max"), back.view("view")), (Some(5), Some(true), Some(v)));
        assert_eq!(back.str("folder"), Some("C:\\Users\\x\\Pictures Algebraic"));
        assert_eq!(back.style("explore."), style);
        assert_eq!(back.style("video."), Style::default());
        assert_eq!(back.get::<usize>("missing"), None);
    }
}
