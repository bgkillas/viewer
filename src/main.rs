use egui::{CentralPanel, Color32, Key, Pos2, Rect, TextureHandle, TextureOptions, Vec2, pos2};
use eyre::{ContextCompat, eyre};
use image::{ImageReader, RgbImage};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::{env, fs};
const CHUNK: u32 = 16384;
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_fullscreen(true),
        ..Default::default()
    };
    eframe::run_native("viewer", options, Box::new(|_cc| Ok(Box::new(App::new()?))))
}
#[derive(PartialEq, Clone)]
struct Chapter {
    major: usize,
    minor: Option<usize>,
}
impl PartialOrd for Chapter {
    fn partial_cmp(&self, other: &Chapter) -> Option<Ordering> {
        match self.major.cmp(&other.major) {
            Ordering::Equal => match (self.minor, other.minor) {
                (Some(a), Some(b)) => a.partial_cmp(&b),
                (Some(_), None) => Some(Ordering::Greater),
                (None, Some(_)) => Some(Ordering::Less),
                (None, None) => Some(Ordering::Equal),
            },
            Ordering::Greater => Some(Ordering::Greater),
            Ordering::Less => Some(Ordering::Less),
        }
    }
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

impl PartialOrd for Page {
    fn partial_cmp(&self, other: &Page) -> Option<Ordering> {
        match self.chapter.partial_cmp(&other.chapter).unwrap() {
            Ordering::Equal => match (self.page, other.page) {
                (Some(a), Some(b)) => a.partial_cmp(&b),
                (Some(_), None) => Some(Ordering::Greater),
                (None, Some(_)) => Some(Ordering::Less),
                (None, None) => Some(Ordering::Equal),
            },
            Ordering::Greater => Some(Ordering::Greater),
            Ordering::Less => Some(Ordering::Less),
        }
    }
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

enum Textures {
    Some(Vec<TextureHandle>),
    One(TextureHandle),
}

struct App {
    data: PathBuf,
    image_path: PathBuf,
    images: HashMap<usize, Textures>,
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
            pages.sort_by(|a, b| a.partial_cmp(b).unwrap());
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
        let (width, height) = img.dimensions();
        if height > CHUNK {
            let mut texs = Vec::new();
            for (i, start_y) in (0..height).step_by(CHUNK as usize).enumerate() {
                let actual_height = (start_y + CHUNK).min(height) - start_y;
                let img =
                    RgbImage::from_fn(width, actual_height, |x, y| *img.get_pixel(x, start_y + y));
                let color_image = egui::ColorImage::from_rgb(
                    [img.width() as usize, img.height() as usize],
                    img.as_flat_samples().as_slice(),
                );
                let tex = ui.ctx().load_texture(
                    format!("{}_{}", p, i),
                    color_image,
                    TextureOptions::NEAREST,
                );
                texs.push(tex);
            }
            self.images.insert(num, Textures::Some(texs));
        } else {
            let color_image = egui::ColorImage::from_rgb(
                [width as usize, height as usize],
                img.as_flat_samples().as_slice(),
            );
            let tex = ui
                .ctx()
                .load_texture(p.to_string(), color_image, TextureOptions::NEAREST);
            self.images.insert(num, Textures::One(tex));
        }
        Ok(())
    }
    fn update_cache(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) -> eyre::Result<()> {
        let range = self.current.saturating_sub(2)..(self.current + 3).min(self.pages.len());
        let mut to_remove = Vec::new();
        for (i, t) in &self.images {
            if !range.contains(i) {
                to_remove.push(*i);
                match t {
                    Textures::One(_) => ctx.forget_image(&self.pages[*i].to_string()),
                    Textures::Some(l) => {
                        let p = self.pages[*i].to_string();
                        for i in 0..l.len() {
                            ctx.forget_image(&format!("{}_{}", p, i));
                        }
                    }
                }
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
        Ok(())
    }
    fn save_path(&self) -> eyre::Result<()> {
        Ok(fs::write(
            &self.data,
            self.pages[self.current].to_string().as_bytes(),
        )?)
    }
    fn main(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) -> eyre::Result<()> {
        self.update_cache(ui, ctx)?;
        ui.input(|i| {
            if i.key_pressed(Key::Z) {
                if self.current != 0 {
                    self.current -= 1;
                    self.save_path()
                } else {
                    Ok(())
                }
            } else if i.key_pressed(Key::C) {
                if self.current != self.pages.len() - 1 {
                    self.current += 1;
                    self.save_path()
                } else {
                    Ok(())
                }
            } else {
                Ok(())
            }
        })?;
        let painter = ui.painter();
        match self.images.get(&self.current).unwrap() {
            Textures::One(image) => {
                let size = image.size();
                let window = ctx.input(|i| i.screen_rect);
                let scale = if !self.is_list && size[1] as f32 > window.height() {
                    window.height() / size[1] as f32
                } else {
                    1.0
                };
                let rect = Rect::from_min_size(
                    Pos2::new(window.width() / 2.0 - size[0] as f32 / 2.0 * scale, 0.0),
                    Vec2::new(size[0] as f32 * scale, size[1] as f32 * scale),
                );
                painter.image(
                    image.id(),
                    rect,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            Textures::Some(l) => {
                for (i, image) in l.iter().enumerate() {
                    let size = image.size();
                    let window = ctx.input(|i| i.screen_rect);
                    let scale = if !self.is_list && size[1] as f32 > window.height() {
                        window.height() / size[1] as f32
                    } else {
                        1.0
                    };
                    let rect = Rect::from_min_size(
                        Pos2::new(
                            window.width() / 2.0 - size[0] as f32 / 2.0 * scale,
                            (i * CHUNK as usize) as f32,
                        ),
                        Vec2::new(size[0] as f32 * scale, size[1] as f32 * scale),
                    );
                    painter.image(
                        image.id(),
                        rect,
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
            }
        }
        Ok(())
    }
}
