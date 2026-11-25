//! Загрузка SVG-иконок в textures для egui.
//!
//! Иконки хранятся в `resources/icons/*.svg` и рендерятся через `resvg`
//! в `egui::ColorImage`, после чего загружаются в egui texture.

use eframe::egui;
use std::collections::HashMap;
use std::path::PathBuf;

/// Хранилище SVG-иконок с готовыми texture.
pub struct SdaIcons {
    textures: HashMap<String, egui::TextureHandle>,
}

impl SdaIcons {
    pub fn new(ctx: &egui::Context) -> Self {
        let mut textures = HashMap::new();
        let icons_dir = Self::icons_dir();
        if let Ok(entries) = std::fs::read_dir(&icons_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                    continue;
                };
                if ext != "svg" {
                    continue;
                }
                let Some(name) = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
                else {
                    continue;
                };
                if let Some(handle) = Self::load(ctx, &path) {
                    textures.insert(name, handle);
                }
            }
        }
        Self { textures }
    }

    pub fn get(&self, name: &str) -> Option<&egui::TextureHandle> {
        self.textures.get(name)
    }

    pub fn get_or_load(&mut self, ctx: &egui::Context, name: &str) -> Option<egui::TextureHandle> {
        if let Some(h) = self.textures.get(name) {
            return Some(h.clone());
        }
        let path = Self::icons_dir().join(format!("{}.svg", name));
        let handle = Self::load(ctx, &path)?;
        self.textures.insert(name.to_string(), handle.clone());
        Some(handle)
    }

    fn icons_dir() -> PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let p = dir.join("resources").join("icons");
                if p.is_dir() {
                    return p;
                }
            }
        }
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("icons")
    }

    fn load(ctx: &egui::Context, path: &std::path::Path) -> Option<egui::TextureHandle> {
        let svg = std::fs::read_to_string(path).ok()?;
        let opt = resvg::usvg::Options::default();
        let tree = resvg::usvg::Tree::from_str(&svg, &opt).ok()?;
        let size = tree.size();
        let width = size.width().max(1.0) as u32;
        let height = size.height().max(1.0) as u32;
        let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
        let mut pixmap_data = pixmap.data_mut();
        let mut pm = resvg::tiny_skia::PixmapMut::from_bytes(&mut pixmap_data, width, height)?;
        resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pm);
        let src = pm.as_ref().data().to_vec();
        let mut rgba = Vec::with_capacity(src.len());
        for p in src.chunks_exact(4) {
            let a = p[3] as f32 / 255.0;
            let r = if a > 0.0 { p[0] as f32 / (255.0 * a) } else { 0.0 };
            let g = if a > 0.0 { p[1] as f32 / (255.0 * a) } else { 0.0 };
            let b = if a > 0.0 { p[2] as f32 / (255.0 * a) } else { 0.0 };
            rgba.extend_from_slice(&[
                (r * 255.0) as u8,
                (g * 255.0) as u8,
                (b * 255.0) as u8,
                (a * 255.0) as u8,
            ]);
        }
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            &rgba,
        );
        Some(ctx.load_texture(
            format!("sda_icon_{}", path.file_stem().and_then(|n| n.to_str()).unwrap_or("icon")),
            image,
            Default::default(),
        ))
    }
}
