mod sandbox;

use std::sync::Mutex;

use js_sys::Uint8Array;
use lazy_static::lazy_static;
use sandbox::Sandbox;
use typst::eval::Tracer;
use wasm_bindgen::prelude::*;

lazy_static! {
    static ref SANDBOX: Mutex<Sandbox> = Mutex::new(Sandbox::new());
}

#[wasm_bindgen]
pub fn render_typst(source: &str) -> Vec<String> {
    console_error_panic_hook::set_once();
    let sandbox = SANDBOX.lock().unwrap();
    let res = render(&sandbox, source);
    res.unwrap_or(vec!["compilation failed".to_string()])
}

#[wasm_bindgen]
pub fn set_fonts(font_files: Box<[JsValue]>) {
    console_error_panic_hook::set_once();
    let font_files: Vec<Vec<u8>> = font_files
        .into_iter()
        .map(|it| Uint8Array::new(&it).to_vec())
        .collect();
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

fn render(sandbox: &Sandbox, source: &str) -> Result<Vec<String>, String> {
    let world = sandbox.with_source(source.to_string());
    let mut tracer = Tracer::default();
    let document = typst::compile(&world, &mut tracer).map_err(|diags| "compilation failed")?;
    let warnings = tracer.warnings();
    

    if document.pages.is_empty() {
        return Err("no pages in rendered output".to_string());
    }
    let pages = document
        .pages
        .iter()
        .map(|page| typst_svg::svg(&page.frame))
        .collect();
    Ok(pages)
}

#[test]
fn test() {
    let input = "#line(length: 100pt)";

    let result = render_typst(input);
    println!("{result:?}");
    assert!(false);
}
