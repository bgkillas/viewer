use eframe::emath::pos2;
use egui::{CentralPanel, Color32, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use eyre::{ContextCompat, eyre};
use image::ImageReader;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::{env, fs};
fn main() -> eframe::Result {
    eframe::run_native(
        "viewer",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(App::new()?))),
    )
}
#[derive(PartialEq, Clone)]
struct Chapter {
    major: usize,
    minor: Option<usize>,
}

impl Display for Chapter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}{}", self.major, self.minor.unwrap_or(0))
    }
}

#[derive(PartialEq, Clone)]
struct Page {
    chapter: Chapter,
    page: Option<usize>,
}

impl Display for Page {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}",
            self.chapter,
            self.page.map(|p| format!("-{:03}", p)).unwrap_or_default()
        )
    }
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
            .show(ctx, |ui| self.main(ctx, ui).unwrap());
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
            let raw = fs::read_to_string(&data)?.trim().to_string();
            let is_list = !raw.contains('-');
            let current = Page::parse(&raw, is_list)?;
            let images = Default::default();
            let mut pages = Vec::new();
            for p in fs::read_dir(&image_path)? {
                pages.push(Page::parse(
                    p?.path().file_name().unwrap().to_str().unwrap(),
                    is_list,
                )?)
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
    fn get_path(&self, page: &Page) -> PathBuf {
        self.image_path.join(page.to_string())
    }
    fn get_img(&mut self, ui: &mut egui::Ui, num: usize) -> eyre::Result<()> {
        let p = &self.pages[num];
        let img = ImageReader::open(self.get_path(p))?
            .with_guessed_format()?
            .decode()?
            .to_rgb8();
        let color_image = egui::ColorImage::from_rgb(
            [img.width() as usize, img.height() as usize],
            img.as_flat_samples().as_slice(),
        );
        let tex = ui
            .ctx()
            .load_texture(p.to_string(), color_image, TextureOptions::NEAREST);
        self.images.insert(num, tex);
        Ok(())
    }
    fn main(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) -> eyre::Result<()> {
        {
            let range = self.current.saturating_sub(1)..(self.current + 2).min(self.pages.len());
            let mut to_remove = Vec::new();
            for i in self.images.keys() {
                if !range.contains(i) {
                    to_remove.push(*i);
                    ctx.forget_image(&self.pages[*i].to_string());
                }
            }
            for i in to_remove {
                self.images.remove(&i);
            }
            for i in range {
                if !self.images.contains_key(&i) {
                    self.get_img(ui, i)?
                }
            }
        }
        let painter = ui.painter();
        let image = self.images.get(&self.current).unwrap();
        let size = image.size();
        let rect = Rect::from_min_size(
            Pos2::new(
                ctx.input(|i| i.screen_rect).width() / 2.0 - size[0] as f32 / 2.0,
                0.0,
            ),
            Vec2::new(size[0] as f32, size[1] as f32),
        );
        painter.image(
            image.id(),
            rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        Ok(())
    }
}
