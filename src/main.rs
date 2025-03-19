use egui::{
    CentralPanel, Color32, CursorIcon, FontData, FontDefinitions, FontFamily, FontId, Key, Painter,
    Pos2, Rect, TextureHandle, TextureOptions, Vec2, pos2,
};
use eyre::{ContextCompat, eyre};
use image::{ImageReader, RgbImage};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::{JoinHandle, spawn};
use std::{env, fs};
const CHUNK: u32 = 16384;
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        ..Default::default()
    };
    eframe::run_native(
        "viewer",
        options,
        Box::new(|cc| {
            let mut fonts = FontDefinitions::default();
            fonts.font_data.insert(
                "terminus".to_owned(),
                Arc::from(FontData::from_static(include_bytes!(
                    "/usr/share/fonts/TTF/TerminessNerdFontMono-Regular.ttf"
                ))),
            );
            fonts
                .families
                .get_mut(&FontFamily::Monospace)
                .unwrap()
                .insert(0, "terminus".to_owned());
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(App::new()?))
        }),
    )
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
    image_tasks: HashMap<usize, JoinHandle<eyre::Result<Vec<egui::ColorImage>>>>,
    pages: Vec<Page>,
    current: usize,
    is_list: bool,
    x: f32,
    y: f32,
    zoom: f32,
    dont_save: bool,
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
                image_tasks: Default::default(),
                dont_save: false,
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
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
                    TextureOptions::LINEAR,
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
                .load_texture(p.to_string(), color_image, TextureOptions::LINEAR);
            self.images.insert(num, Textures::One(tex));
        }
        Ok(())
    }
    fn insert_images(&mut self, ui: &mut egui::Ui, num: usize, mut images: Vec<egui::ColorImage>) {
        if images.len() == 1 {
            let color_image = images.remove(0);
            let tex = ui.ctx().load_texture(
                self.pages[num].to_string(),
                color_image,
                TextureOptions::LINEAR,
            );
            self.images.insert(num, Textures::One(tex));
        } else {
            let mut texs = Vec::new();
            for color_image in images {
                let tex = ui.ctx().load_texture(
                    self.pages[num].to_string(),
                    color_image,
                    TextureOptions::LINEAR,
                );
                texs.push(tex);
            }
            self.images.insert(num, Textures::Some(texs));
        }
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
        let mut to_remove = Vec::new();
        for i in self.image_tasks.keys() {
            if !range.contains(i) {
                to_remove.push(*i);
            }
        }
        for i in to_remove {
            if let Some(t) = self.image_tasks.remove(&i) {
                t.join().unwrap()?;
            }
        }
        for i in range {
            if !self.images.contains_key(&i) {
                if i == self.current {
                    if let Some(task) = self.image_tasks.remove(&i) {
                        self.insert_images(ui, i, task.join().unwrap()?)
                    } else {
                        self.get_img(ui, i)?;
                    }
                } else if !self.image_tasks.contains_key(&i) {
                    let page = self.pages[i].clone();
                    let image_path = self.image_path.clone();
                    self.image_tasks
                        .insert(i, spawn(move || get_imgs(page, image_path)));
                }
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
    fn display(&self, image: &TextureHandle, painter: &Painter, ctx: &egui::Context) {
        let size = image.size();
        let window = ctx.input(|i| i.screen_rect);
        let scale = if !self.is_list && size[1] as f32 > window.height() {
            window.height() / size[1] as f32
        } else {
            1.0
        };
        let rect = Rect::from_min_size(
            Pos2::new(
                window.width() / 2.0 - size[0] as f32 / 2.0 * scale * self.zoom
                    + self.x * self.zoom,
                self.y * self.zoom,
            ),
            Vec2::new(
                size[0] as f32 * scale * self.zoom,
                size[1] as f32 * scale * self.zoom,
            ),
        );
        painter.image(
            image.id(),
            rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    fn main(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) -> eyre::Result<()> {
        ctx.set_cursor_icon(CursorIcon::None);
        ui.input(|i| {
            if i.key_pressed(Key::Z) {
                if self.current > 0 {
                    (self.x, self.y, self.zoom) = (0.0, 0.0, 1.0);
                    self.current -= 1;
                    self.dont_save = false;
                    self.save_path()
                } else {
                    Ok(())
                }
            } else if i.key_pressed(Key::C) {
                if self.current + 1 < self.pages.len() {
                    (self.x, self.y, self.zoom) = (0.0, 0.0, 1.0);
                    self.current += 1;
                    if !self.is_list || self.current != self.pages.len() - 1 {
                        self.dont_save = false;
                        self.save_path()
                    } else {
                        Ok(())
                    }
                } else {
                    Ok(())
                }
            } else {
                if i.key_pressed(Key::A) {
                    //TODO shift+*, and snap to edges
                    self.x += 64.0 / self.zoom
                }
                if i.key_pressed(Key::D) {
                    self.x -= 64.0 / self.zoom
                }
                if i.key_pressed(Key::W) {
                    self.y += 64.0 / self.zoom
                }
                if i.key_pressed(Key::S) {
                    self.y -= 64.0 / self.zoom
                }
                if i.key_pressed(Key::Q) {
                    self.zoom /= 1.5
                }
                if i.key_pressed(Key::E) {
                    self.zoom *= 1.5
                }
                Ok(())
            }
        })?;
        self.update_cache(ui, ctx)?;
        let painter = ui.painter();
        match self.images.get(&self.current).unwrap() {
            Textures::One(image) => self.display(image, painter, ctx),
            Textures::Some(l) => {
                for image in l {
                    self.display(image, painter, ctx);
                    self.y += CHUNK as f32;
                }
                self.y -= (CHUNK as usize * l.len()) as f32
            }
        }
        let rect = ui.max_rect();
        let bottom_left = Rect::from_min_size(
            rect.left_bottom() - egui::vec2(0.0, 48.0),
            egui::vec2(rect.width(), 48.0),
        );
        ui.painter().text(
            bottom_left.left_bottom(),
            egui::Align2::LEFT_BOTTOM,
            if self.is_list {
                let h = match self.images.get(&self.current).unwrap() {
                    Textures::One(tex) => tex.size()[1],
                    Textures::Some(tex) => {
                        tex.last().unwrap().size()[1] + CHUNK as usize * (tex.len() - 1)
                    }
                };
                let p = ((-self.y) as usize * 100) / h;
                if p > 90 && self.current == self.pages.len() - 1 && !self.dont_save {
                    self.dont_save = true;
                    self.save_path()?;
                }
                format!(
                    "{:03}\n{}/{}",
                    p,
                    self.pages[self.current],
                    self.pages.last().unwrap()
                )
            } else {
                format!(
                    "{}/{}\n{}/{}\n{:03}/{:03}",
                    self.current + 1,
                    self.pages.len(),
                    self.pages[self.current].chapter,
                    self.pages.last().unwrap().chapter,
                    self.pages[self.current].page.unwrap(),
                    self.pages.last().unwrap().page.unwrap()
                )
            },
            FontId::monospace(16.0),
            Color32::WHITE,
        );

        Ok(())
    }
}
fn get_imgs(p: Page, image_path: PathBuf) -> eyre::Result<Vec<egui::ColorImage>> {
    let img = ImageReader::open(image_path.join(p.to_string()))?
        .with_guessed_format()?
        .decode()?
        .to_rgb8();
    let (width, height) = img.dimensions();
    if height > CHUNK {
        let mut texs = Vec::new();
        for start_y in (0..height).step_by(CHUNK as usize) {
            let actual_height = (start_y + CHUNK).min(height) - start_y;
            let img =
                RgbImage::from_fn(width, actual_height, |x, y| *img.get_pixel(x, start_y + y));
            texs.push(egui::ColorImage::from_rgb(
                [img.width() as usize, img.height() as usize],
                img.as_flat_samples().as_slice(),
            ))
        }
        Ok(texs)
    } else {
        Ok(vec![egui::ColorImage::from_rgb(
            [width as usize, height as usize],
            img.as_flat_samples().as_slice(),
        )])
    }
}
