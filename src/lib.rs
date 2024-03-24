mod sandbox;

use std::{num::NonZeroUsize, sync::Mutex};

use js_sys::Uint8Array;
use lazy_static::lazy_static;
use sandbox::Sandbox;
use typst::{eval::Tracer, visualize::Color};
use wasm_bindgen::prelude::*;


lazy_static!{
    static ref SANDBOX: Mutex<Sandbox> = Mutex::new(Sandbox::new());
}

#[wasm_bindgen]
pub fn render_typst(source: &str) -> String {
    let sandbox = SANDBOX.lock().unwrap();
    let res = render(&sandbox, source);
    res.unwrap_or("compilation failed".to_string())
}

#[wasm_bindgen]
pub fn add_fonts(font_files: Box<[JsValue]>) {
    assert!(font_files.iter().all(|it| it.is_array()));
    let font_files: Vec<Vec<u8>> = font_files.into_iter().map(|it| Uint8Array::from(it.clone()).to_vec()).collect();
    let font_files: Vec<&[u8]> = font_files.iter().map(|it| &it[..]).collect();
    let mut sandbox = SANDBOX.lock().unwrap();
    sandbox.set_fonts(font_files);
}

fn panic_to_string(panic: &dyn std::any::Any) -> String {
	let inner = panic
		.downcast_ref::<&'static str>()
		.copied()
		.or_else(|| panic.downcast_ref::<String>().map(String::as_str))
		.unwrap_or("Box<dyn Any>");
	format!("panicked at '{inner}'")
}

fn render(sandbox: &Sandbox, source: &str) -> Result<String, String> {
    let world = sandbox.with_source(source.to_string());
    let mut tracer = Tracer::default();
    let document = typst::compile(&world, &mut tracer).map_err(|diags| "compilation failed")?;
    let warnings = tracer.warnings();

    let frame = &document.pages.first().ok_or("no pages in rendered output")?.frame;
    let more_pages = NonZeroUsize::new(document.pages.len().saturating_sub(1));

    let pixels_per_point = 0; // TODO 
    let transparent = Color::from_u8(0, 0, 0, 0);
    let svg = typst_svg::svg(frame);

    Ok(svg)
}

#[test]
fn test() {
    let input = "#line(length: 100pt)";

    let result = render_typst(input);
    println!("{result}");
    assert!(false);
}