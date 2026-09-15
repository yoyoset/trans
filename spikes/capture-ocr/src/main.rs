//! M0 验证工具：按窗口截图 → 裁剪/放大 → Windows OCR，输出识别文本与耗时。
//!
//! 用法：
//!   spike list                                  列出可截取的窗口
//!   spike capture <标题关键字|进程名> [选项]     截取窗口一帧并 OCR
//!   spike file <png 路径> [选项]                对已有截图做 OCR（便于反复调预处理）
//!
//! 选项：
//!   --region x,y,w,h   裁剪区域（帧像素坐标）
//!   --scale  f         放大倍数，默认 1.0
//!   --lang   tag       OCR 语言，默认 en-US
//!   --out    dir       保存处理后的图片，便于人工核对

mod capture;
mod image_ops;
mod ocr;

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{bail, Context, Result};

use image_ops::{Bgra, Region};

struct Options {
    region: Option<Region>,
    scale: f32,
    lang: String,
    out: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        bail!("缺少子命令：list | capture <窗口> | file <png>");
    };
    match cmd.as_str() {
        "list" => capture::print_windows(),
        "capture" => {
            let target = args.get(1).context("capture 需要窗口标题关键字或进程名")?;
            let opts = parse_options(&args[2..])?;
            let started = Instant::now();
            let frame = capture::grab_one_frame(target)?;
            println!("capture: {}x{} in {} ms", frame.width, frame.height, started.elapsed().as_millis());
            run_pipeline(frame, &opts)
        }
        "file" => {
            let path = args.get(1).context("file 需要 png 路径")?;
            let opts = parse_options(&args[2..])?;
            run_pipeline(image_ops::load_png(path)?, &opts)
        }
        other => bail!("未知子命令：{other}"),
    }
}

fn run_pipeline(frame: Bgra, opts: &Options) -> Result<()> {
    let processed = image_ops::preprocess(frame, opts.region, opts.scale)?;
    if let Some(dir) = &opts.out {
        let path = image_ops::save_png(&processed, dir)?;
        println!("saved: {}", path.display());
    }
    let result = ocr::recognize(&processed, &opts.lang)?;
    println!("ocr: {} lines in {} ms ({}x{})", result.lines.len(), result.elapsed_ms, processed.width, processed.height);
    for line in &result.lines {
        println!("  [{:>5.0},{:>5.0} {:>4.0}x{:<3.0}] {}", line.x, line.y, line.w, line.h, line.text);
    }
    Ok(())
}

fn parse_options(args: &[String]) -> Result<Options> {
    let mut opts = Options { region: None, scale: 1.0, lang: "en-US".into(), out: None };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().with_context(|| format!("{flag} 缺少参数值"))?;
        match flag.as_str() {
            "--region" => opts.region = Some(Region::parse(value)?),
            "--scale" => opts.scale = value.parse().context("--scale 应为数字")?,
            "--lang" => opts.lang = value.clone(),
            "--out" => opts.out = Some(PathBuf::from(value)),
            other => bail!("未知选项：{other}"),
        }
    }
    Ok(opts)
}
