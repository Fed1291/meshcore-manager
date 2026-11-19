//! The program's icon (assets/icon.svg, the one drawing to change): on Windows it is drawn at
//! every size Windows shows (16 to 256 px) into an .ico, and that goes into mcmanager.exe
//! (Explorer, the taskbar, a shortcut). Nothing to install: the drawing is done here, in Rust.
//! Other systems: nothing (a terminal program has no icon there; the .svg serves as it is).

use std::error::Error;
use std::path::{Path, PathBuf};

/// The sizes Windows picks from (list, taskbar, high-DPI screens, large icons)
const SIZES: [u32; 9] = [16, 20, 24, 32, 40, 48, 64, 128, 256];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icon.svg");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let ico = out.join("mcmanager.ico");
    if let Err(e) = make_ico(&manifest.join("assets").join("icon.svg"), &ico) {
        panic!("the icon could not be made from assets/icon.svg: {e}");
    }
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(&ico.to_string_lossy());
    if let Err(e) = resource.compile() {
        panic!("the icon could not be put into the program (Windows resource compiler, rc.exe): {e}");
    }
}

/// Draws the .svg at every size and writes them into one .ico
fn make_ico(svg: &Path, ico: &Path) -> Result<(), Box<dyn Error>> {
    let data = std::fs::read(svg)?;
    let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default())?;
    let mut icons = ico::IconDir::new(ico::ResourceType::Icon);
    for size in SIZES {
        let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).ok_or("bad icon size")?;
        let scale = size as f32 / tree.size().width();
        resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
        // the drawing's pixels have their colour multiplied by their opacity; an .ico wants them plain
        let mut rgba = pixmap.data().to_vec();
        for px in rgba.chunks_exact_mut(4) {
            let alpha = px[3] as u32;
            if alpha > 0 && alpha < 255 {
                for c in &mut px[..3] {
                    *c = ((*c as u32 * 255 + alpha / 2) / alpha).min(255) as u8;
                }
            }
        }
        let image = ico::IconImage::from_rgba_data(size, size, rgba);
        icons.add_entry(ico::IconDirEntry::encode(&image)?);
    }
    icons.write(std::fs::File::create(ico)?)?;
    Ok(())
}
