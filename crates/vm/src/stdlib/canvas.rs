use error::error::VyprError;
use crate::value::Value;
use crate::stdlib::error;
use crate::heap::Heap;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};

thread_local! {
    static CANVAS_WIDTH: RefCell<usize> = const { RefCell::new(0) };
    static CANVAS_HEIGHT: RefCell<usize> = const { RefCell::new(0) };
    static CANVAS_PIXELS: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

pub fn create_module(heap: &mut Heap) -> Value {
    let mut exports = HashMap::new();

    exports.insert("init".to_string(), Value::allocate_native(heap, "init".to_string(), canvas_init));
    exports.insert("set_pixel".to_string(), Value::allocate_native(heap, "set_pixel".to_string(), canvas_set_pixel));
    exports.insert("save".to_string(), Value::allocate_native(heap, "save".to_string(), canvas_save));
    exports.insert("draw_rect".to_string(), Value::allocate_native(heap, "draw_rect".to_string(), canvas_draw_rect));
    exports.insert("draw_circle".to_string(), Value::allocate_native(heap, "draw_circle".to_string(), canvas_draw_circle));

    Value::allocate_module(heap, "canvas".to_string(), exports)
}

fn canvas_init(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 2 {
        return Err(error("R014", format!("init() takes exactly 2 arguments, got {}", args.len())));
    }

    if args[0].is_int() && args[1].is_int() {
        let width = args[0].as_int() as usize;
        let height = args[1].as_int() as usize;

        CANVAS_WIDTH.with(|cw| *cw.borrow_mut() = width);
        CANVAS_HEIGHT.with(|ch| *ch.borrow_mut() = height);
        
        CANVAS_PIXELS.with(|cp| {
            *cp.borrow_mut() = vec![0; width * height * 3];
        });
        Ok(Value::none())
    } else {
        Err(error("R002", "arguments to 'init()' must be of type 'int'"))
    }
}

fn canvas_set_pixel(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 5 {
        return Err(error("R014", format!("set_pixel() takes exactly 5 arguments, got {}", args.len())));
    }

    if args[0].is_int() && args[1].is_int() && args[2].is_int() && args[3].is_int() && args[4].is_int() {
        let x = args[0].as_int() as usize;
        let y = args[1].as_int() as usize;
        let r = args[2].as_int() as u8;
        let g = args[3].as_int() as u8;
        let b = args[4].as_int() as u8;

        CANVAS_WIDTH.with(|cw| {
            CANVAS_HEIGHT.with(|ch| {
                let width = *cw.borrow();
                let height = *ch.borrow();

                if x < width && y < height {
                    let idx = (y * width + x) * 3;

                    CANVAS_PIXELS.with(|cp| {
                        let mut pixels = cp.borrow_mut();
                        pixels[idx] = r;
                        pixels[idx + 1] = g;
                        pixels[idx + 2] = b;
                    });
                }
            })
        });

        Ok(Value::none())
    } else {
        Err(error("R002", "arguments to 'set_pixel()' must be of type 'int'"))
    }
}

fn canvas_save(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("save() takes exactly 1 argument, got {}", args.len())));
    }

    if let Some(filename) = args[0].as_str() {
        CANVAS_WIDTH.with(|cw| {
            CANVAS_HEIGHT.with(|ch| {
                CANVAS_PIXELS.with(|cp| {
                    let width = *cw.borrow();
                    let height = *ch.borrow();
                    let pixels = cp.borrow();

                    if let Ok(file) = File::create(filename) {
                        let mut writer = BufWriter::new(file);
                        let _ = writeln!(writer, "P3\n{} {}\n255", width, height);
                        
                        for chunk in pixels.chunks(3) {
                            let _ = writeln!(writer, "{} {} {}", chunk[0], chunk[1], chunk[2]);
                        }
                    }
                })
            })
        });

        Ok(Value::none())
    } else {
        Err(error("R002", "argument to 'save()' must be of type 'str'"))
    }
}

fn canvas_draw_rect(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 7 {
        return Err(error("R014", format!("draw_rect() takes exactly 7 arguments, got {}", args.len())));
    }
    
    if args[0].is_int() && args[1].is_int() && args[2].is_int() && args[3].is_int() && args[4].is_int() && args[5].is_int() && args[6].is_int() {
        let start_x = args[0].as_int() as usize;
        let start_y = args[1].as_int() as usize;
        let w = args[2].as_int() as usize;
        let h = args[3].as_int() as usize;
        let r = args[4].as_int() as u8;
        let g = args[5].as_int() as u8;
        let b = args[6].as_int() as u8;

        CANVAS_WIDTH.with(|cw| {
            CANVAS_HEIGHT.with(|ch| {
                let width = *cw.borrow();
                let height = *ch.borrow();

                CANVAS_PIXELS.with(|cp| {
                    let mut pixels = cp.borrow_mut();
                    for y in start_y..(start_y + h) {
                        for x in start_x..(start_x + w) {
                            if x < width && y < height {
                                let idx = (y * width + x) * 3;
                                pixels[idx] = r;
                                pixels[idx + 1] = g;
                                pixels[idx + 2] = b;
                            }
                        }
                    }
                });
            })
        });

        Ok(Value::none())
    } else {
        Err(error("R002", "arguments to 'draw_rect()' must be of type 'int'"))
    }
}

fn canvas_draw_circle(_heap: &mut Heap, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, VyprError> {
    if args.len() != 6 {
        return Err(error("R014", format!("draw_circle() takes exactly 6 arguments, got {}", args.len())));
    }
    
    if args[0].is_int() && args[1].is_int() && args[2].is_int() && args[3].is_int() && args[4].is_int() && args[5].is_int() {
        let cx = args[0].as_int() as isize;
        let cy = args[1].as_int() as isize;
        let rad = args[2].as_int() as isize;
        let r = args[3].as_int() as u8;
        let g = args[4].as_int() as u8;
        let b = args[5].as_int() as u8;
        
        let rad_sq = rad * rad;

        CANVAS_WIDTH.with(|cw| {
            CANVAS_HEIGHT.with(|ch| {
                let width = *cw.borrow() as isize;
                let height = *ch.borrow() as isize;

                CANVAS_PIXELS.with(|cp| {
                    let mut pixels = cp.borrow_mut();
                    
                    let min_y = (cy - rad).max(0);
                    let max_y = (cy + rad).min(height - 1);
                    let min_x = (cx - rad).max(0);
                    let max_x = (cx + rad).min(width - 1);

                    for y in min_y..=max_y {
                        for x in min_x..=max_x {
                            let dx = x - cx;
                            let dy = y - cy;
                            
                            if dx * dx + dy * dy <= rad_sq {
                                let idx = ((y as usize) * (width as usize) + (x as usize)) * 3;
                                pixels[idx] = r;
                                pixels[idx + 1] = g;
                                pixels[idx + 2] = b;
                            }
                        }
                    }
                });
            })
        });

        Ok(Value::none())
    } else {
        Err(error("R002", "arguments to 'draw_circle()' must be of type 'int'"))
    }
}
