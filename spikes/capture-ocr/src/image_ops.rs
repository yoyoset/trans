//! BGRA 像素缓冲的裁剪、放大、PNG 读写。

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use image::{imageops, RgbaImage};

/// 连续 BGRA8 像素，无行填充。Windows OCR 的 SoftwareBitmap 直接吃这个格式。
pub struct Bgra {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Clone, Copy)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Region {
    pub fn parse(s: &str) -> Result<Self> {
        let parts: Vec<u32> = s.split(',').map(|p| p.trim().parse()).collect::<Result<_, _>>().context("--region 格式应为 x,y,w,h")?;
        let [x, y, w, h] = parts[..] else { bail!("--region 需要 4 个数") };
        Ok(Self { x, y, w, h })
    }
}

pub fn preprocess(frame: Bgra, region: Option<Region>, scale: f32) -> Result<Bgra> {
    let mut img = to_rgba(frame)?;
    if let Some(r) = region {
        if r.x + r.w > img.width() || r.y + r.h > img.height() {
            bail!("裁剪区域超出画面 {}x{}", img.width(), img.height());
        }
        img = imageops::crop_imm(&img, r.x, r.y, r.w, r.h).to_image();
    }
    if (scale - 1.0).abs() > f32::EPSILON {
        let w = (img.width() as f32 * scale).round() as u32;
        let h = (img.height() as f32 * scale).round() as u32;
        img = imageops::resize(&img, w, h, imageops::FilterType::CatmullRom);
    }
    Ok(from_rgba(img))
}

pub fn load_png(path: &str) -> Result<Bgra> {
    let img = image::open(path).with_context(|| format!("读取 {path} 失败"))?.to_rgba8();
    Ok(from_rgba(img))
}

pub fn save_png(frame: &Bgra, dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis();
    let path = dir.join(format!("frame-{millis}.png"));
    to_rgba(Bgra { width: frame.width, height: frame.height, pixels: frame.pixels.clone() })?.save(&path)?;
    Ok(path)
}

fn to_rgba(frame: Bgra) -> Result<RgbaImage> {
    let mut px = frame.pixels;
    swap_red_blue(&mut px);
    RgbaImage::from_raw(frame.width, frame.height, px).context("像素缓冲长度与尺寸不符")
}

fn from_rgba(img: RgbaImage) -> Bgra {
    let (width, height) = img.dimensions();
    let mut pixels = img.into_raw();
    swap_red_blue(&mut pixels);
    Bgra { width, height, pixels }
}

fn swap_red_blue(px: &mut [u8]) {
    px.chunks_exact_mut(4).for_each(|c| c.swap(0, 2));
}
