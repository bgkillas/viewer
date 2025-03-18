use eframe::emath::pos2;
use egui::load::TexturePoll;
use egui::{CentralPanel, Color32, Pos2, Rect, SizeHint, TextureHandle, TextureOptions, Vec2};
use eyre::{ContextCompat, eyre};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{env, fs};
fn main() -> eframe::Result {
    eframe::run_native(
        "viewer",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(App::new().unwrap()))),
    )
}
#[derive(PartialEq, Clone)]
struct Chapter {
    major: usize,
    minor: Option<usize>,
}

impl Chapter {
    fn to_string(&self) -> String {
        format!("{:04}{}", self.major, self.minor.unwrap_or(0))
    }
}

#[derive(PartialEq, Clone)]
struct Page {
    chapter: Chapter,
    page: Option<usize>,
}

impl Page {
    fn parse(raw: &str, is_list: bool) -> eyre::Result<Self> {
        let chars = raw.chars();
        let major = chars.clone().take(4).collect::<String>().parse::<usize>()?;
        let minor = chars
            .clone()
            .skip(4)
            .take(1)
            .collect::<String>()
            .parse::<usize>()?;
        let minor = if minor == 0 { None } else { Some(minor) };
        let chapter = Chapter { major, minor };
        let page = if is_list {
            None
        } else {
            Some(chars.skip(6).collect::<String>().parse::<usize>()?)
        };
        Ok(Page { chapter, page })
    }
    fn to_string(&self) -> String {
        format!(
            "{}{}",
            self.chapter.to_string(),
            self.page
                .map(|p| format!("-{:03}", p))
                .unwrap_or(String::new())
        )
    }
}

struct App {
    data: PathBuf,
    image_path: PathBuf,
    images: HashMap<usize, TextureHandle>,
    pages: Vec<Page>,
    current: usize,
    is_list: bool,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default()
            .frame(egui::Frame::default().fill(Color32::from_rgb(0, 0, 0)))
            .show(ctx, |ui| self.main(ui));
    }
}
impl App {
    fn new() -> eyre::Result<Self> {
        let name = env::current_dir()?
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let data = Path::new("/home/.p").join(&name);
        let image_path = Path::new("/home/.m").join(&name);
        if !fs::exists(&data)? || !fs::exists(&image_path)? {
            Err(eyre!("bad paths"))
        } else {
            let raw = fs::read_to_string(&data)?
                .trim()
                .chars()
                .collect::<String>();
            let is_list = !raw.contains('-');
            let current = Page::parse(&raw, is_list)?;
            let mut images = Default::default();
            let mut pages = Vec::new();
            for p in fs::read_dir(&image_path)? {
                pages.push(Page::parse(p?.path().to_str().unwrap(), is_list)?)
            }
            let current = pages
                .iter()
                .position(|p| p == &current)
                .wrap_err("page not found")?;
            Ok(App {
                images,
                image_path,
                data,
                is_list,
                pages,
                current,
            })
        }
    }
    fn main(&mut self, ui: &mut egui::Ui) -> eyre::Result<()> {
                if !self.images.iter().any(|(p, _)| p == &self.current) {
                    let img = ImageReader::
            let color_image =
                egui::ColorImage::from_rgba_unmultiplied(size, img.as_flat_samples().as_slice());
            let tex = ui
                .ctx()
                .load_texture(name, color_image, TextureOptions::NEAREST);
            self.images.insert(self.current, tex);
        }
                let painter = ui.painter();
                let image = self.images.get(&self.current).unwrap();
                let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), image.size().unwrap());
                painter.image(
                    image.texture_id().unwrap(),
                    rect,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::BLACK,
                );
        Ok(())
    }
}