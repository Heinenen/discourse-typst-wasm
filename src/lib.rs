mod sandbox;

use std::num::NonZeroUsize;

use sandbox::Sandbox;
use typst::{eval::Tracer, visualize::Color};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern {
    pub fn log(s: &str);
}

#[wasm_bindgen]
pub fn greet(num: &str) -> String {
    format!("Hello, {num}!").to_string()
}

#[wasm_bindgen]
pub fn render_typst(source: &str) -> String {
    let sandbox = Sandbox::new();
    let res = render(&sandbox, source);
    res.unwrap_or("compilation failed".to_string())

}

fn setup() {
    let sandbox = Sandbox::new();
    
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
    let input = "hello world!";

    let result = render_typst(input);
    println!("{result}");
    assert!(false);
}